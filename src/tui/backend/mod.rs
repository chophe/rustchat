mod embedded;
mod remote;

pub use embedded::EmbeddedBackend;
pub use remote::RemoteBackend;

use crate::config::GlobalConfig;
use crate::tui::event::BackendEvent;
use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedReceiver;

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub name: String,
}

#[async_trait]
pub trait ChatBackend: Send {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>>;
    fn load_session(&mut self, name: &str) -> Result<()>;
    async fn send_message(&mut self, text: &str) -> Result<()>;
    fn subscribe(&self) -> UnboundedReceiver<BackendEvent>;
    fn abort(&self);
    fn set_model(&mut self, model_id: &str) -> Result<()>;
    fn set_role(&mut self, name: &str) -> Result<()>;
    fn use_session(&mut self, name: Option<&str>) -> Result<()>;
    fn sync_header(&self) -> (String, String);
    fn sync_messages(&self) -> Vec<crate::tui::state::MessageView>;
    fn sync_sidebar(&self) -> Vec<String>;
    fn list_models(&self) -> Vec<String>;
    fn list_roles(&self) -> Vec<String>;
    fn exit(&mut self) -> Result<()>;
}

pub enum Backend {
    Embedded(EmbeddedBackend),
    Remote(RemoteBackend),
}

#[async_trait]
impl ChatBackend for Backend {
    fn list_sessions(&self) -> Result<Vec<SessionMeta>> {
        match self {
            Self::Embedded(b) => b.list_sessions(),
            Self::Remote(b) => b.list_sessions(),
        }
    }

    fn load_session(&mut self, name: &str) -> Result<()> {
        match self {
            Self::Embedded(b) => b.load_session(name),
            Self::Remote(b) => b.load_session(name),
        }
    }

    async fn send_message(&mut self, text: &str) -> Result<()> {
        match self {
            Self::Embedded(b) => b.send_message(text).await,
            Self::Remote(b) => b.send_message(text).await,
        }
    }

    fn subscribe(&self) -> UnboundedReceiver<BackendEvent> {
        match self {
            Self::Embedded(b) => b.subscribe(),
            Self::Remote(b) => b.subscribe(),
        }
    }

    fn abort(&self) {
        match self {
            Self::Embedded(b) => b.abort(),
            Self::Remote(b) => b.abort(),
        }
    }

    fn set_model(&mut self, model_id: &str) -> Result<()> {
        match self {
            Self::Embedded(b) => b.set_model(model_id),
            Self::Remote(b) => b.set_model(model_id),
        }
    }

    fn set_role(&mut self, name: &str) -> Result<()> {
        match self {
            Self::Embedded(b) => b.set_role(name),
            Self::Remote(b) => b.set_role(name),
        }
    }

    fn use_session(&mut self, name: Option<&str>) -> Result<()> {
        match self {
            Self::Embedded(b) => b.use_session(name),
            Self::Remote(b) => b.use_session(name),
        }
    }

    fn sync_header(&self) -> (String, String) {
        match self {
            Self::Embedded(b) => b.sync_header(),
            Self::Remote(b) => b.sync_header(),
        }
    }

    fn sync_messages(&self) -> Vec<crate::tui::state::MessageView> {
        match self {
            Self::Embedded(b) => b.sync_messages(),
            Self::Remote(b) => b.sync_messages(),
        }
    }

    fn sync_sidebar(&self) -> Vec<String> {
        match self {
            Self::Embedded(b) => b.sync_sidebar(),
            Self::Remote(b) => b.sync_sidebar(),
        }
    }

    fn list_models(&self) -> Vec<String> {
        match self {
            Self::Embedded(b) => b.list_models(),
            Self::Remote(b) => b.list_models(),
        }
    }

    fn list_roles(&self) -> Vec<String> {
        match self {
            Self::Embedded(b) => b.list_roles(),
            Self::Remote(b) => b.list_roles(),
        }
    }

    fn exit(&mut self) -> Result<()> {
        match self {
            Self::Embedded(b) => b.exit(),
            Self::Remote(b) => b.exit(),
        }
    }
}

pub fn create_backend(config: GlobalConfig, tui_url: Option<String>) -> Result<Backend> {
    match tui_url {
        Some(url) => Ok(Backend::Remote(RemoteBackend::new(url)?)),
        None => Ok(Backend::Embedded(EmbeddedBackend::new(config)?)),
    }
}
