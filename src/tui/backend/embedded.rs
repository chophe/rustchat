use super::{ChatBackend, SessionMeta};
use crate::client::{
    call_chat_completions, call_chat_completions_streaming_tui, list_models, ModelType, SseEvent,
};
use crate::config::{Config, GlobalConfig, Input};
use crate::tui::event::BackendEvent;
use crate::tui::state::MessageView;
use crate::utils::{create_abort_signal, AbortSignal};

use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;

use std::sync::Mutex;

pub struct EmbeddedBackend {
    config: GlobalConfig,
    abort_signal: AbortSignal,
    event_tx: UnboundedSender<BackendEvent>,
    event_rx: Mutex<Option<UnboundedReceiver<BackendEvent>>>,
    chat_task: Option<JoinHandle<()>>,
}

impl EmbeddedBackend {
    pub fn new(config: GlobalConfig) -> Result<Self> {
        let (event_tx, event_rx) = unbounded_channel();
        Ok(Self {
            config,
            abort_signal: create_abort_signal(),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
            chat_task: None,
        })
    }

    async fn run_chat(&self, text: String) {
        let config = self.config.clone();
        let abort_signal = self.abort_signal.clone();
        let event_tx = self.event_tx.clone();

        let result: Result<()> = async {
            let line = text.trim();
            if line.is_empty() {
                return Ok(());
            }

            if let Some(exit) = Self::handle_command(&config, &abort_signal, line, &event_tx).await? {
                if exit {
                    let _ = event_tx.send(BackendEvent::Exit);
                }
                return Ok(());
            }

            let mut input = Input::from_str(&config, line, None);
            input.use_embeddings(abort_signal.clone()).await?;

            while config.read().is_compressing_session() {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }

            Self::chat_loop(&config, &abort_signal, &event_tx, input).await
        }
        .await;

        if let Err(err) = result {
            let _ = event_tx.send(BackendEvent::Error(err.to_string()));
        }
    }

    #[async_recursion::async_recursion]
    async fn chat_loop(
        config: &GlobalConfig,
        abort_signal: &AbortSignal,
        event_tx: &UnboundedSender<BackendEvent>,
        input: Input,
    ) -> Result<()> {
        let client = input.create_client()?;
        config.write().before_chat_completion(&input)?;

        let (output, tool_results) = if input.stream() {
            let (tx, mut rx) = unbounded_channel();
            let event_tx = event_tx.clone();
            let forward = tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    match event {
                        SseEvent::Text(text) => {
                            let _ = event_tx.send(BackendEvent::StreamDelta(text));
                        }
                        SseEvent::Done => break,
                    }
                }
            });

            let result =
                call_chat_completions_streaming_tui(&input, client.as_ref(), abort_signal.clone(), tx)
                    .await;
            let _ = forward.await;
            result?
        } else {
            let (text, tool_results) =
                call_chat_completions(&input, false, false, client.as_ref(), abort_signal.clone())
                    .await?;
            if !text.is_empty() {
                let _ = event_tx.send(BackendEvent::StreamDelta(text.clone()));
            }
            (text, tool_results)
        };

        config
            .write()
            .after_chat_completion(&input, &output, &tool_results)?;

        if !tool_results.is_empty() {
            for result in &tool_results {
                let _ = event_tx.send(BackendEvent::ToolCall(result.call.clone()));
            }
            let _ = event_tx.send(BackendEvent::Done {
                text: output.clone(),
            });
            return Self::chat_loop(
                config,
                abort_signal,
                event_tx,
                input.merge_tool_results(output, tool_results),
            )
            .await;
        }

        Config::maybe_autoname_session(config.clone());
        Config::maybe_compress_session(config.clone());
        let _ = event_tx.send(BackendEvent::Done { text: output });
        let _ = event_tx.send(BackendEvent::SessionUpdated);
        Ok(())
    }

    async fn handle_command(
        config: &GlobalConfig,
        abort_signal: &AbortSignal,
        line: &str,
        event_tx: &UnboundedSender<BackendEvent>,
    ) -> Result<Option<bool>> {
        if !line.starts_with('.') {
            return Ok(None);
        }

        let (cmd, args) = match line.split_once(' ') {
            Some((cmd, args)) => (cmd, Some(args.trim())),
            None => (line, None),
        };

        match cmd {
            ".help" => {
                let _ = event_tx.send(BackendEvent::Info(
                    "Commands: .help .model .role .session .exit | Ctrl+P palette".into(),
                ));
            }
            ".exit" => return Ok(Some(true)),
            ".model" => match args {
                Some(name) => config.write().set_model(name)?,
                None => bail!("Usage: .model <name>"),
            },
            ".role" => match args {
                Some(name) => {
                    if !Config::has_role(name) {
                        config.write().new_role(name)?;
                    }
                    config.write().use_role(name)?;
                }
                None => bail!("Usage: .role <name>"),
            },
            ".session" => {
                config.write().use_session(args)?;
                Config::maybe_autoname_session(config.clone());
            }
            ".agent" => {
                crate::repl::run_repl_command(config, abort_signal.clone(), line).await?;
            }
            ".rag" => {
                Config::use_rag(config, args, abort_signal.clone()).await?;
            }
            _ if line.starts_with('.') => {
                crate::repl::run_repl_command(config, abort_signal.clone(), line).await?;
            }
            _ => return Ok(None),
        }
        Ok(Some(false))
    }
}

#[async_trait]
impl ChatBackend for EmbeddedBackend {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        let config = self.config.read();
        let dir = config.sessions_dir();
        Ok(config
            .list_sessions()
            .into_iter()
            .map(|name| {
                let updated = std::fs::metadata(config.session_file(&name))
                    .and_then(|m| m.modified())
                    .ok()
                    .or_else(|| {
                        // Autoname sessions live under the `_` subdirectory.
                        std::fs::metadata(dir.join("_").join(format!("{name}.yaml")))
                            .and_then(|m| m.modified())
                            .ok()
                    });
                SessionMeta { name, updated }
            })
            .collect())
    }

    fn load_session(&mut self, name: &str) -> Result<()> {
        self.config.write().use_session(Some(name))
    }

    fn delete_session(&mut self, name: &str) -> Result<()> {
        let path = self.config.read().session_file(name);
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        // If the deleted session is the active one, fall back to a temp session.
        let current = self
            .config
            .read()
            .session
            .as_ref()
            .map(|s| s.name().to_string());
        if current.as_deref() == Some(name) {
            self.config.write().use_session(None)?;
        }
        Ok(())
    }

    async fn send_message(&mut self, text: &str) -> Result<()> {
        if self.chat_task.as_ref().is_some_and(|t| !t.is_finished()) {
            bail!("Already generating a response");
        }
        let text = text.to_string();
        let this = self.clone_for_task();
        let handle = tokio::spawn(async move {
            this.run_chat(text).await;
        });
        self.chat_task = Some(handle);
        Ok(())
    }

    fn subscribe(&self) -> UnboundedReceiver<BackendEvent> {
        self.event_rx
            .lock()
            .unwrap()
            .take()
            .expect("subscribe called more than once")
    }

    fn abort(&self) {
        self.abort_signal.set_ctrlc();
    }

    fn set_model(&mut self, model_id: &str) -> Result<()> {
        self.config.write().set_model(model_id)
    }

    fn set_role(&mut self, name: &str) -> Result<()> {
        self.config.write().use_role(name)
    }

    fn use_session(&mut self, name: Option<&str>) -> Result<()> {
        self.config.write().use_session(name)
    }

    fn sync_header(&self) -> (String, String) {
        let config = self.config.read();
        (
            config.render_prompt_left(),
            config.render_prompt_right(),
        )
    }

    fn sync_messages(&self) -> Vec<MessageView> {
        let config = self.config.read();
        if let Some(session) = config.session.as_ref() {
            return session
                .messages()
                .iter()
                .map(MessageView::from_message)
                .collect();
        }
        vec![]
    }

    fn sync_sidebar(&self) -> Vec<String> {
        let config = self.config.read();
        let mut lines = vec![];
        lines.push(format!("Model: {}", config.model.id()));
        if let Some(role) = config.role.as_ref() {
            lines.push(format!("Role: {}", role.name()));
        }
        if let Some(session) = config.session.as_ref() {
            lines.push(format!("Session: {}", session.name()));
            let (tokens, percent) = session.tokens_usage();
            lines.push(format!("Tokens: {tokens} ({percent:.0}%)"));
        }
        if let Some(rag) = config.rag.as_ref() {
            lines.push(format!("RAG: {}", rag.name()));
            if let Ok(sources) = Config::rag_sources(&self.config) {
                if !sources.is_empty() {
                    lines.push("Sources:".into());
                    for line in sources.lines().take(8) {
                        lines.push(format!("  {line}"));
                    }
                }
            }
        }
        if let Some(agent) = config.agent.as_ref() {
            lines.push(format!("Agent: {}", agent.name()));
        }
        lines
    }

    fn list_models(&self) -> Vec<String> {
        list_models(&self.config.read(), ModelType::Chat)
            .into_iter()
            .map(|m| m.id().to_string())
            .collect()
    }

    fn list_roles(&self) -> Vec<String> {
        Config::list_roles(true)
    }

    fn exit(&mut self) -> Result<()> {
        self.config.write().exit_session()
    }
}

impl EmbeddedBackend {
    fn clone_for_task(&self) -> Self {
        Self {
            config: self.config.clone(),
            abort_signal: self.abort_signal.clone(),
            event_tx: self.event_tx.clone(),
            event_rx: Mutex::new(None),
            chat_task: None,
        }
    }
}
