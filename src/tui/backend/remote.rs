use super::{ChatBackend, SessionMeta};
use crate::tui::event::BackendEvent;
use crate::tui::state::MessageView;

use anyhow::Result;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest_eventsource::{Event, RequestBuilderExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RemoteEvent {
    StreamDelta { text: String },
    Done { text: String },
    Error { message: String },
    SessionUpdated,
}

use std::sync::Mutex;

pub struct RemoteBackend {
    base_url: String,
    client: reqwest::Client,
    event_tx: UnboundedSender<BackendEvent>,
    event_rx: Mutex<Option<UnboundedReceiver<BackendEvent>>>,
    messages: Vec<MessageView>,
    header_left: String,
    header_right: String,
    sidebar_lines: Vec<String>,
    abort_tx: Option<UnboundedSender<()>>,
}

impl RemoteBackend {
    pub fn new(base_url: String) -> Result<Self> {
        let base_url = base_url.trim_end_matches('/').to_string();
        let (event_tx, event_rx) = unbounded_channel();
        Ok(Self {
            base_url,
            client: reqwest::Client::new(),
            event_tx,
            event_rx: Mutex::new(Some(event_rx)),
            messages: vec![],
            header_left: "remote".into(),
            header_right: String::new(),
            sidebar_lines: vec![],
            abort_tx: None,
        })
    }

    async fn fetch_sessions(&self) -> Result<Vec<SessionMeta>> {
        let url = format!("{}/v1/tui/sessions", self.base_url);
        let resp = self.client.get(&url).send().await?;
        let names: Vec<String> = resp.json().await?;
        Ok(names.into_iter().map(|name| SessionMeta { name }).collect())
    }
}

#[async_trait]
impl ChatBackend for RemoteBackend {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.fetch_sessions())
        })
    }

    fn load_session(&mut self, name: &str) -> Result<()> {
        self.header_left = format!("session:{name}");
        Ok(())
    }

    async fn send_message(&mut self, text: &str) -> Result<()> {
        let url = format!("{}/v1/tui/chat", self.base_url);
        let (abort_tx, mut abort_rx) = unbounded_channel();
        self.abort_tx = Some(abort_tx);

        self.messages.push(MessageView {
            role: crate::client::MessageRole::User,
            text: text.to_string(),
            streaming: false,
            tool_calls: vec![],
        });

        let client = self.client.clone();
        let event_tx = self.event_tx.clone();
        let body = serde_json::json!({ "message": text });

        let handle = tokio::spawn(async move {
            let builder = client.post(&url).json(&body);
            let mut es = match builder.eventsource() {
                Ok(es) => es,
                Err(err) => {
                    let _ = event_tx.send(BackendEvent::Error(err.to_string()));
                    return;
                }
            };

            loop {
                tokio::select! {
                    _ = abort_rx.recv() => {
                        es.close();
                        break;
                    }
                    event = es.next() => {
                        match event {
                            Some(Ok(Event::Open)) => {}
                            Some(Ok(Event::Message(message))) => {
                                if let Ok(remote) = serde_json::from_str::<RemoteEvent>(&message.data) {
                                    match remote {
                                        RemoteEvent::StreamDelta { text } => {
                                            let _ = event_tx.send(BackendEvent::StreamDelta(text));
                                        }
                                        RemoteEvent::Done { text } => {
                                            let _ = event_tx.send(BackendEvent::Done { text });
                                            break;
                                        }
                                        RemoteEvent::Error { message } => {
                                            let _ = event_tx.send(BackendEvent::Error(message));
                                            break;
                                        }
                                        RemoteEvent::SessionUpdated => {
                                            let _ = event_tx.send(BackendEvent::SessionUpdated);
                                        }
                                    }
                                }
                            }
                            Some(Err(err)) => {
                                let _ = event_tx.send(BackendEvent::Error(err.to_string()));
                                break;
                            }
                            None => break,
                        }
                    }
                }
            }
        });

        let _ = handle.await;
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
        if let Some(tx) = &self.abort_tx {
            let _ = tx.send(());
        }
    }

    fn set_model(&mut self, model_id: &str) -> Result<()> {
        self.header_right = format!("model:{model_id}");
        Ok(())
    }

    fn set_role(&mut self, name: &str) -> Result<()> {
        self.sidebar_lines.push(format!("Role: {name}"));
        Ok(())
    }

    fn use_session(&mut self, name: Option<&str>) -> Result<()> {
        match name {
            Some(name) => self.load_session(name),
            None => Ok(()),
        }
    }

    fn sync_header(&self) -> (String, String) {
        (self.header_left.clone(), self.header_right.clone())
    }

    fn sync_messages(&self) -> Vec<MessageView> {
        self.messages.clone()
    }

    fn sync_sidebar(&self) -> Vec<String> {
        self.sidebar_lines.clone()
    }

    fn list_models(&self) -> Vec<String> {
        vec![]
    }

    fn list_roles(&self) -> Vec<String> {
        vec![]
    }

    fn exit(&mut self) -> Result<()> {
        Ok(())
    }
}
