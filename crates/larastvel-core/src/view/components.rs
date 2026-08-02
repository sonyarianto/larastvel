//! # Blade Components & Slots
//!
//! A lightweight, linear (non-nested) implementation of Laravel's Blade
//! components with named slots:
//!
//! | Blade | Larastvel |
//! |---|---|
//! | `<x-card title="Hi">…</x-card>` | `@component('components/card.html', {title: "Hi"}) … @endcomponent` |
//! | `<x-slot:header>…</x-slot>` | captured as the `header` variable |
//! | `@slot('header') … @endslot` | captured as the `header` variable |
//! | `{{ $slot }}` | the component's default (unnamed) body |
//!
//! Like [`super::blade::compile`], this is a regex-based pre-processor — it
//! does not parse nested components or nested slots.

use std::collections::HashMap;

use once_cell::sync::Lazy;
use regex::Regex;

use super::blade;
use super::ViewError;

/// A named slot captured from a component body.
#[derive(Debug, Clone)]
pub struct Slot {
    pub name: String,
    pub content: String,
}

/// A `@component('template', {attrs}) … @endcomponent` block.
#[derive(Debug, Clone)]
pub struct ComponentBlock {
    /// Template name relative to the views root (e.g. `components/card.html`).
    pub template: String,
    /// Attribute variables passed into the component template.
    pub attributes: HashMap<String, String>,
    /// The block body with any slot blocks removed.
    pub body: String,
}

/// A class-style Blade component (Laravel's `App\View\Components`).
pub trait Component: Send + Sync {
    /// Template name relative to the views root.
    fn view(&self) -> &str;
    /// Extra data merged into the render context.
    fn data(&self) -> HashMap<String, serde_json::Value> {
        HashMap::new()
    }
}

// ---------------------------------------------------------------------------
// Slot extraction
// ---------------------------------------------------------------------------

static MODERN_SLOT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?s)<x-slot:([a-zA-Z0-9_-]+)\s*>(.*?)</x-slot\s*>").unwrap());

static CLASSIC_SLOT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?s)@slot\s*\(\s*'([^']+)'\s*\)(.*?)@endslot"#).unwrap());

/// Pull named slot blocks out of a component body.
///
/// Both the modern `<x-slot:name>…</x-slot>` and the classic
/// `@slot('name') … @endslot` syntax are supported. Slot blocks are removed
/// from the body (so the remainder becomes the default `$slot`) and returned
/// in order of appearance.
pub fn extract_slots(input: &str) -> (String, Vec<Slot>) {
    let mut slots = Vec::new();

    let body = MODERN_SLOT
        .replace_all(input, |caps: &regex::Captures| {
            slots.push(Slot {
                name: caps[1].to_string(),
                content: caps[2].to_string(),
            });
            String::new()
        })
        .to_string();

    let body = CLASSIC_SLOT
        .replace_all(&body, |caps: &regex::Captures| {
            slots.push(Slot {
                name: caps[1].to_string(),
                content: caps[2].to_string(),
            });
            String::new()
        })
        .to_string();

    (body, slots)
}

// ---------------------------------------------------------------------------
// @component block splitting
// ---------------------------------------------------------------------------

static COMPONENT_BLOCK: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?s)@component\s*\(\s*'([^']+)'\s*(?:,\s*\{([^}]*)\}\s*)?\)(.*?)@endcomponent")
        .unwrap()
});

/// Split a compiled template into `@component` blocks and the surrounding
/// page. Each block is replaced in the returned page with a placeholder
/// token (`__LARASTVEL_COMPONENT_N__`) that the render pipeline later swaps
/// for the rendered component HTML.
pub fn split_components(input: &str) -> (String, Vec<ComponentBlock>) {
    let mut blocks = Vec::new();
    let mut index = 0usize;

    let page = COMPONENT_BLOCK
        .replace_all(input, |caps: &regex::Captures| {
            let template = caps[1].to_string();
            let attributes = parse_attributes(caps.get(2).map(|m| m.as_str()).unwrap_or(""));
            let body = caps[3].to_string();
            let token = format!("__LARASTVEL_COMPONENT_{}__", index);
            index += 1;
            blocks.push(ComponentBlock {
                template,
                attributes,
                body,
            });
            format!("{{{{ {} }}}}", token)
        })
        .to_string();

    (page, blocks)
}

/// Parse a `{ key: "value", key2: 42 }` attribute list into a map. Values
/// are kept as raw strings (numbers, booleans and quoted strings all pass
/// through; quotes are stripped).
fn parse_attributes(input: &str) -> HashMap<String, String> {
    let mut attributes = HashMap::new();
    static ATTR: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"([a-zA-Z_][a-zA-Z0-9_-]*)\s*:\s*('([^']*)'|"([^"]*)"|([^,}\s]+))"#).unwrap()
    });
    for caps in ATTR.captures_iter(input) {
        let key = caps[1].to_string();
        let value = caps
            .get(3)
            .or_else(|| caps.get(4))
            .or_else(|| caps.get(5))
            .map(|m| m.as_str())
            .unwrap_or("")
            .to_string();
        attributes.insert(key, value);
    }
    attributes
}

// ---------------------------------------------------------------------------
// Render pipeline
// ---------------------------------------------------------------------------

/// Render a component block: capture and render its slots, then render the
/// component template with `slot` + named slot + attribute variables.
pub(crate) fn render_component_block(
    tera: &mut tera::Tera,
    views_root: &std::path::Path,
    context: &tera::Context,
    block: &ComponentBlock,
) -> Result<String, ViewError> {
    let (slot_body, slots) = extract_slots(&block.body);

    let mut component_context = context.clone();
    for (key, value) in &block.attributes {
        component_context.insert(key, value);
    }
    for slot in slots {
        let html = render_fragment(tera, context, &slot.content)?;
        component_context.insert(&slot.name, &html);
    }
    let default_slot = render_fragment(tera, context, &slot_body)?;
    component_context.insert("slot", &default_slot);

    let template_path = views_root.join(&block.template);
    let raw = std::fs::read_to_string(&template_path).map_err(|e| ViewError::Render {
        template: block.template.clone(),
        source: Box::new(e),
    })?;

    tera.render_str(&blade::compile(&raw), &component_context)
        .map_err(|e| ViewError::Render {
            template: block.template.clone(),
            source: Box::new(e),
        })
}

/// Compile + render a template fragment (used for slot bodies).
fn render_fragment(
    tera: &mut tera::Tera,
    context: &tera::Context,
    input: &str,
) -> Result<String, ViewError> {
    let compiled = blade::compile(input);
    tera.render_str(&compiled, context)
        .map_err(|e| ViewError::Render {
            template: "<fragment>".to_string(),
            source: Box::new(e),
        })
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_modern_slots() {
        let (body, slots) =
            extract_slots("<x-slot:header>Welcome</x-slot><x-slot:footer>Bye</x-slot>Body");
        assert_eq!(body, "Body");
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0].name, "header");
        assert_eq!(slots[0].content, "Welcome");
        assert_eq!(slots[1].name, "footer");
        assert_eq!(slots[1].content, "Bye");
    }

    #[test]
    fn extracts_classic_slots() {
        let (body, slots) = extract_slots("@slot('title') <b>Hi</b> @endslot Body");
        assert_eq!(body.trim(), "Body");
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].name, "title");
        assert_eq!(slots[0].content.trim(), "<b>Hi</b>");
    }

    #[test]
    fn extracts_slots_with_whitespace_tags() {
        let (body, slots) = extract_slots("<x-slot:header >Hello</x-slot >Body");
        assert_eq!(body, "Body");
        assert_eq!(slots[0].name, "header");
        assert_eq!(slots[0].content, "Hello");
    }

    #[test]
    fn no_slots_passthrough() {
        let input = "<div>Plain body</div>";
        let (body, slots) = extract_slots(input);
        assert_eq!(body, input);
        assert!(slots.is_empty());
    }

    #[test]
    fn splits_component_blocks_with_placeholders() {
        let (page, blocks) = split_components(
            "Before @component('components/card.html', { title: \"Hi\" })Body@endcomponent After",
        );
        assert!(page.contains("Before"));
        assert!(page.contains("After"));
        assert!(page.contains("{{ __LARASTVEL_COMPONENT_0__ }}"));
        assert!(!page.contains("@component"));
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].template, "components/card.html");
        assert_eq!(blocks[0].attributes.get("title").unwrap(), "Hi");
        assert_eq!(blocks[0].body, "Body");
    }

    #[test]
    fn splits_component_blocks_without_attributes() {
        let (page, blocks) =
            split_components("@component('components/alert.html')Oops@endcomponent");
        assert!(page.contains("{{ __LARASTVEL_COMPONENT_0__ }}"));
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].attributes.is_empty());
        assert_eq!(blocks[0].body, "Oops");
    }

    #[test]
    fn parses_attributes() {
        let attrs = parse_attributes("type: \"danger\", count: 42, active: true");
        assert_eq!(attrs.get("type").unwrap(), "danger");
        assert_eq!(attrs.get("count").unwrap(), "42");
        assert_eq!(attrs.get("active").unwrap(), "true");
        assert_eq!(attrs.len(), 3);
    }

    #[test]
    fn renders_component_block_pipeline() {
        let dir = std::env::temp_dir().join(format!("larastvel_view_test_{}", uuid_v4()));
        let views = dir.join("resources/views");
        std::fs::create_dir_all(views.join("components")).unwrap();
        std::fs::write(
            views.join("components/card.html"),
            "<section><h1>{{ title }}</h1><aside>{{ header }}</aside><p>{{ slot }}</p></section>",
        )
        .unwrap();

        let mut tera = tera::Tera::parse(views.join("**/*.html").to_str().unwrap()).unwrap();
        let mut context = tera::Context::new();
        context.insert("user", "Ada");

        let page = "@component('components/card.html', { title: \"Stats\" })\
             <x-slot:header>Hi {{ user }}</x-slot>Body text@endcomponent";
        let (page, blocks) = split_components(page);
        let mut rendered = page.clone();
        for (i, block) in blocks.iter().enumerate() {
            let html = render_component_block(&mut tera, &views, &context, block).unwrap();
            let marker = format!("{{{{ __LARASTVEL_COMPONENT_{}__ }}}}", i);
            rendered = rendered.replace(&marker, &html);
        }
        rendered = tera.render_str(&rendered, &context).unwrap();

        assert!(rendered.contains("<h1>Stats</h1>"), "{rendered}");
        assert!(rendered.contains("<aside>Hi Ada</aside>"), "{rendered}");
        assert!(rendered.contains("<p>Body text</p>"), "{rendered}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn uuid_v4() -> String {
        uuid::Uuid::new_v4().to_string()
    }
}
