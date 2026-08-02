# Views & Templating

Larastvel uses Tera for templating with Blade-style convenience directives.

## Configuration

Configure the view engine in `config/view.toml`:

```toml
engine = "tera"
paths = ["resources/views"]
```

## Rendering Views

```rust
use larastvel_core::view::ViewFactory;

let factory = ViewFactory::new(&config);
let html = factory.render("welcome", &ctx).await?;
```

Or use the `view` shorthand on routes:

```rust
router.view("/welcome", "welcome");
```

## Blade-Style Directives

Tera templates support Blade-inspired directives:

```html
<!-- resources/views/layouts/app.html -->
<!DOCTYPE html>
<html>
<head>
    <title>{% block title %}Larastvel{% endblock %}</title>
</head>
<body>
    @auth
        <p>Welcome, {{ user.name }}</p>
    @endauth

    @guest
        <p>Please <a href="/login">log in</a></p>
    @endguest

    @csrf
    @method('PUT')

    {% block content %}{% endblock %}

    @error('email')
        <p>{{ message }}</p>
    @enderror
</body>
</html>
```

Supported directives: `@auth`, `@endauth`, `@guest`, `@endguest`, `@csrf`, `@method`, `@error`, `@enderror`.

## Components & Slots

Laravel-style components with named slots (Laravel's `x-slot`). Three syntaxes are equivalent:

```html
<!-- resources/views/components/card.html -->
<section>
    <h1>{{ title }}</h1>
    <aside>{{ header }}</aside>
    <p>{{ slot }}</p>
</section>
```

```html
<!-- x-component syntax -->
<x-card title="Stats">
    <x-slot:header>Welcome {{ user.name }}</x-slot>
    Body content
</x-card>

<!-- classic @component syntax -->
@component('components/card.html', { title: "Stats" })
    @slot('header') Welcome {{ user.name }} @endslot
    Body content
@endcomponent
```

Both render `components/card.html` with `title`, `header`, and `slot` (the default, unnamed body) variables available. Slot content is compiled and rendered with the page context before being passed to the component — so `{{ user.name }}` inside a slot works.

Self-closing components are supported: `<x-icon name="heart" />`.

Class-style components implement the `Component` trait:

```rust
use larastvel_core::{Component, ViewFactory};
use std::collections::HashMap;

struct Alert { title: String }

impl Component for Alert {
    fn view(&self) -> &str {
        "components/alert.html"
    }
    fn data(&self) -> HashMap<String, serde_json::Value> {
        HashMap::from([("title".into(), serde_json::json!(self.title))])
    }
}

let html = factory.render_component(&alert, ctx).await?;
```

Components are non-nested: slot/component blocks end at the first closing tag (same linear limitation as the directive compiler).

## Vite Asset Bundling

Larastvel integrates with Vite via the manifest file:

```rust
use larastvel_core::support::Vite;

let tags = Vite::asset("resources/js/app.js");
// Generates <script> and <link> tags from manifest
```

Configure Vite in your project root with a `vite.config.js`.
