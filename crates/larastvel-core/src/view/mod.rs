use std::sync::Arc;

use axum::response::Html;
use serde::Serialize;
use tera::{Context, Tera};
use tokio::sync::RwLock;

use crate::config::Config;

#[derive(Clone)]
pub struct ViewFactory {
    engine: Arc<RwLock<ViewEngine>>,
}

enum ViewEngine {
    Tera(Tera),
    None,
}

impl ViewFactory {
    pub fn new(config: &Config) -> Self {
        let engine = if config.view.engine == "tera" {
            let glob_pattern = "resources/views/**/*.html";
            if let Ok(tera) = Tera::parse(glob_pattern) {
                ViewEngine::Tera(tera)
            } else {
                ViewEngine::Tera(Tera::default())
            }
        } else {
            ViewEngine::None
        };

        Self {
            engine: Arc::new(RwLock::new(engine)),
        }
    }

    pub async fn render(&self, template: &str, data: impl Serialize) -> Result<String, ViewError> {
        let engine = self.engine.read().await;
        match &*engine {
            ViewEngine::Tera(tera) => {
                let context = Context::from_serialize(data).map_err(|e| ViewError::Render {
                    template: template.to_string(),
                    source: Box::new(e),
                })?;
                let rendered = tera
                    .render(template, &context)
                    .map_err(|e| ViewError::Render {
                        template: template.to_string(),
                        source: Box::new(e),
                    })?;
                Ok(rendered)
            }
            ViewEngine::None => Err(ViewError::NoEngine),
        }
    }

    pub async fn render_html(
        &self,
        template: &str,
        data: impl Serialize,
    ) -> Result<Html<String>, ViewError> {
        self.render(template, data).await.map(Html)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ViewError {
    #[error("No template engine configured")]
    NoEngine,
    #[error("Failed to render template '{template}': {source}")]
    Render {
        template: String,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
