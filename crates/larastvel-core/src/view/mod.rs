use std::path::PathBuf;
use std::sync::Arc;

use axum::response::Html;
use serde::Serialize;
use tera::{Context, Tera};
use tokio::sync::RwLock;

use crate::config::Config;

pub mod blade;
pub mod components;

pub use components::Component;

#[derive(Clone)]
pub struct ViewFactory {
    engine: Arc<RwLock<ViewEngine>>,
    views_root: Arc<PathBuf>,
}

enum ViewEngine {
    Tera(Box<Tera>),
    None,
}

impl ViewFactory {
    pub fn new(config: &Config) -> Self {
        let engine = if config.view.engine == "tera" {
            let glob_pattern = "resources/views/**/*.html";
            let tera = Tera::parse(glob_pattern).unwrap_or_default();

            ViewEngine::Tera(Box::new(tera))
        } else {
            ViewEngine::None
        };

        Self {
            engine: Arc::new(RwLock::new(engine)),
            views_root: Arc::new(PathBuf::from("resources/views")),
        }
    }

    /// Override the views root directory (used in tests).
    pub fn with_views_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.views_root = Arc::new(root.into());
        self
    }

    pub async fn render(&self, template: &str, data: impl Serialize) -> Result<String, ViewError> {
        let mut engine = self.engine.write().await;
        match &mut *engine {
            ViewEngine::Tera(tera) => {
                let mut context = Context::from_serialize(data).map_err(|e| ViewError::Render {
                    template: template.to_string(),
                    source: Box::new(e),
                })?;

                // 1) Read raw template source from disk
                // Tera loads templates from the views root, so we read
                // from the same base path.
                let template_path = self.views_root.join(template);
                let raw =
                    std::fs::read_to_string(&template_path).map_err(|e| ViewError::Render {
                        template: template.to_string(),
                        source: Box::new(e),
                    })?;

                // 2) Compile Blade directives → Tera syntax
                let compiled = blade::compile(&raw);
                let compiled = blade::compile_x_components(&compiled);

                // 3) Render components (with their slots) first, then the page
                let (page, blocks) = components::split_components(&compiled);
                for (i, block) in blocks.iter().enumerate() {
                    let html = components::render_component_block(
                        tera,
                        &self.views_root,
                        &context,
                        block,
                    )?;
                    context.insert(format!("__LARASTVEL_COMPONENT_{}__", i), &html);
                }

                // 4) Render the pre-processed template string via render_str
                let rendered = tera
                    .render_str(&page, &context)
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

    /// Render a class-style component (`Component` trait) to HTML.
    pub async fn render_component(
        &self,
        component: &dyn components::Component,
        data: impl Serialize,
    ) -> Result<Html<String>, ViewError> {
        let mut engine = self.engine.write().await;
        let ViewEngine::Tera(tera) = &mut *engine else {
            return Err(ViewError::NoEngine);
        };

        let mut context = Context::from_serialize(data).map_err(|e| ViewError::Render {
            template: component.view().to_string(),
            source: Box::new(e),
        })?;
        for (key, value) in component.data() {
            context.insert(key, &value);
        }

        let template_path = self.views_root.join(component.view());
        let raw = std::fs::read_to_string(&template_path).map_err(|e| ViewError::Render {
            template: component.view().to_string(),
            source: Box::new(e),
        })?;

        let rendered = tera
            .render_str(&blade::compile(&raw), &context)
            .map_err(|e| ViewError::Render {
                template: component.view().to_string(),
                source: Box::new(e),
            })?;

        Ok(Html(rendered))
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
    use std::collections::HashMap;

    struct Alert {
        title: String,
    }

    impl components::Component for Alert {
        fn view(&self) -> &str {
            "components/alert.html"
        }
        fn data(&self) -> HashMap<String, serde_json::Value> {
            let mut data = HashMap::new();
            data.insert("title".to_string(), serde_json::json!(self.title));
            data
        }
    }

    #[test]
    fn test_blade_module_accessible() {
        // Verify the blade module compiles and exposes compile()
        let result = blade::compile("@csrf");
        assert!(result.contains("csrf_token"));
    }

    #[test]
    fn test_x_components_compile_to_component_blocks() {
        let out = blade::compile_x_components(
            r#"<x-card title="Hi"><x-slot:header>H</x-slot>Body</x-card>"#,
        );
        assert!(out.starts_with("@component('components/card.html', { title: \"Hi\" })"));
        assert!(out.contains("<x-slot:header>H</x-slot>"));
        assert!(out.ends_with("Body@endcomponent"));
    }

    #[test]
    fn test_x_components_self_closing() {
        let out = blade::compile_x_components(r#"<x-icon name="heart" />"#);
        assert_eq!(
            out,
            "@component('components/icon.html', { name: \"heart\" })@endcomponent"
        );
    }

    #[test]
    fn test_x_slot_tags_are_preserved() {
        let out = blade::compile_x_components("<x-slot:header>H</x-slot>");
        assert_eq!(out, "<x-slot:header>H</x-slot>");
    }

    #[test]
    fn test_view_render_passthrough() {
        let config = Config::load(&std::path::PathBuf::from("."));
        let factory = ViewFactory::new(&config);

        // Use render_str via the Tera instance directly for comparison.
        // The ViewFactory expects a template loaded by glob. For this
        // test just verify the factory constructs without panic.
        let _ = factory;
    }

    #[tokio::test]
    async fn test_render_component_pipeline() {
        let dir = std::env::temp_dir().join(format!("larastvel_vf_test_{}", uuid_v4()));
        let views = dir.join("resources/views");
        std::fs::create_dir_all(views.join("components")).unwrap();
        std::fs::write(
            views.join("components/alert.html"),
            "<div class=\"alert\"><h2>{{ title }}</h2>{{ slot }}</div>",
        )
        .unwrap();
        std::fs::write(
            views.join("page.html"),
            r#"<x-alert><x-slot:title>Warning</x-slot>Something happened</x-alert>"#,
        )
        .unwrap();

        let config = Config::load(&std::path::PathBuf::from("."));
        let factory = ViewFactory::new(&config).with_views_root(&views);

        let html = factory
            .render("page.html", serde_json::json!({}))
            .await
            .unwrap();
        assert!(html.contains("<h2>Warning</h2>"));
        assert!(html.contains("Something happened"));
        assert!(!html.contains("x-alert"));

        // Class component path
        let component = Alert {
            title: "Custom".to_string(),
        };
        let out = factory
            .render_component(&component, serde_json::json!({ "slot": "hi" }))
            .await
            .unwrap();
        assert!(out.0.contains("<h2>Custom</h2>"));
        assert!(out.0.contains("hi"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn uuid_v4() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
