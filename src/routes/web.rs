use larastvel_core::{axum, routing::Registrar};

pub fn web(router: &Registrar) {
    router.get("/", || async {
        axum::response::Html(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Larastvel</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: linear-gradient(135deg, #0f172a 0%, #1e293b 100%); min-height: 100vh; display: flex; align-items: center; justify-content: center; color: #e2e8f0; }
        .container { text-align: center; padding: 2rem; }
        h1 { font-size: 4rem; font-weight: 800; background: linear-gradient(135deg, #f59e0b, #ef4444, #ec4899); -webkit-background-clip: text; -webkit-text-fill-color: transparent; margin-bottom: 1rem; }
        p { font-size: 1.25rem; color: #94a3b8; margin-bottom: 2rem; }
        .info { display: flex; gap: 2rem; justify-content: center; flex-wrap: wrap; }
        .card { background: rgba(255,255,255,0.05); border: 1px solid rgba(255,255,255,0.1); border-radius: 1rem; padding: 1.5rem 2rem; backdrop-filter: blur(10px); }
        .card h3 { font-size: 0.875rem; text-transform: uppercase; letter-spacing: 0.05em; color: #64748b; margin-bottom: 0.5rem; }
        .card span { font-size: 1.5rem; font-weight: 700; color: #f59e0b; }
    </style>
</head>
<body>
    <div class="container">
        <h1>Larastvel</h1>
        <p>The Rust framework inspired by Laravel</p>
        <div class="info">
            <div class="card">
                <h3>Version</h3>
                <span>0.2.0</span>
            </div>
            <div class="card">
                <h3>Runtime</h3>
                <span>Axum + Tokio</span>
            </div>
            <div class="card">
                <h3>Database</h3>
                <span>SeaORM</span>
            </div>
        </div>
    </div>
</body>
</html>"#,
        )
    });
}
