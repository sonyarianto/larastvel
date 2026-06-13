use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ViteManifest {
    #[serde(flatten)]
    entries: HashMap<String, ViteEntry>,
}

#[derive(Debug, Deserialize)]
struct ViteEntry {
    file: String,
    css: Option<Vec<String>>,
    src: Option<String>,
    is_entry: Option<bool>,
}

pub struct Vite {
    dev_server_url: String,
    build_path: String,
    manifest: Option<ViteManifest>,
    is_dev: bool,
}

impl Vite {
    pub fn new(_app_url: &str, build_path: &str, is_dev: bool) -> Self {
        Self {
            dev_server_url: "http://localhost:5173".to_string(),
            build_path: build_path.to_string(),
            manifest: None,
            is_dev,
        }
    }

    pub fn load_manifest(&mut self, public_path: &Path) {
        if self.is_dev {
            return;
        }
        let manifest_path = public_path.join(&self.build_path).join("manifest.json");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(manifest_path).ok();
            if let Some(c) = content {
                self.manifest = serde_json::from_str(&c).ok();
            }
        }
    }

    pub fn asset(&self, entry: &str) -> Vec<String> {
        if self.is_dev {
            return vec![format!("{}/{}", self.dev_server_url, entry)];
        }

        let mut assets = vec![];
        if let Some(ref manifest) = self.manifest {
            if let Some(entry_info) = manifest.entries.get(entry) {
                assets.push(format!("/{}/{}", self.build_path, entry_info.file));
                if let Some(ref css) = entry_info.css {
                    for c in css {
                        assets.push(format!("/{}/{}", self.build_path, c));
                    }
                }
            }
        }
        assets
    }

    pub fn js_tags(&self, entry: &str) -> String {
        self.asset(entry)
            .iter()
            .map(|a| {
                if a.ends_with(".js") {
                    format!(r#"<script type="module" src="{}"></script>"#, a)
                } else {
                    String::new()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn css_tags(&self, entry: &str) -> String {
        self.asset(entry)
            .iter()
            .filter(|a| a.ends_with(".css"))
            .map(|a| format!(r#"<link rel="stylesheet" href="{}">"#, a))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
