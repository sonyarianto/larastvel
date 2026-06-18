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

let html = ViewFactory::render("welcome", &ctx)?;
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

## Vite Asset Bundling

Larastvel integrates with Vite via the manifest file:

```rust
use larastvel_core::support::Vite;

let tags = Vite::asset("resources/js/app.js");
// Generates <script> and <link> tags from manifest
```

Configure Vite in your project root with a `vite.config.js`.
