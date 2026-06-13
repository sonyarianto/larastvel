use std::sync::Arc;

use axum::response::Html;
use serde::Serialize;
use tera::{Context, Tera};
use tokio::sync::RwLock;

use crate::config::Config;

pub mod blade;

#[derive(Clone)]
pub struct ViewFactory {
    engine: Arc<RwLock<ViewEngine>>,
}

enum ViewEngine {
    Tera(Box<Tera>),
    None,
}

impl ViewFactory {
    pub fn new(config: &Config) -> Self {
        let engine = if config.view.engine == "tera" {
            let glob_pattern = "resources/views/**/*.html";
            let mut tera = if let Ok(t) = Tera::parse(glob_pattern) {
                t
            } else {
                Tera::default()
            };


            ViewEngine::Tera(Box::new(tera))
        } else {
            ViewEngine::None
        };

        Self {
            engine: Arc::new(RwLock::new(engine)),
        }
    }

    pub async fn render(&self, template: &str, data: impl Serialize) -> Result<String, ViewError> {
        let mut engine = self.engine.write().await;
        match &mut *engine {
            ViewEngine::Tera(tera) => {
                let context = Context::from_serialize(data).map_err(|e| ViewError::Render {
                    template: template.to_string(),
                    source: Box::new(e),
                })?;

                // 1) Read raw template source from disk
                // Tera loads templates from resources/views/, so we read
                // from the same base path.
                let template_path = std::path::Path::new("resources/views").join(template);
                let raw = std::fs::read_to_string(&template_path).map_err(|e| {
                    ViewError::Render {
                        template: template.to_string(),
                        source: Box::new(e),
                    }
                })?;

                // 2) Compile Blade directives → Tera syntax
                let compiled = blade::compile(&raw);

                // 3) Render the pre-processed template string via render_str
                let rendered = tera
                    .render_str(&compiled, &context)
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



// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

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

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blade_module_accessible() {
        // Verify the blade module compiles and exposes compile()
        let result = blade::compile("@csrf");
        assert!(result.contains("csrf_token"));
    }

    #[tokio::test]
    async fn test_view_render_passthrough() {
        let config = Config::load(&std::path::PathBuf::from("."));
        let factory = ViewFactory::new(&config);

        // Use render_str via the Tera instance directly for comparison.
        // The ViewFactory expects a template loaded by glob. For this
        // test just verify the factory constructs without panic.
        let _ = factory;
    }


}
