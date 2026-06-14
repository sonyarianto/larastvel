//! # WebSocket Broadcast Example
//!
//! Demonstrates `NativeBroadcaster` and `ws_handler` — a self-hosted WebSocket
//! broadcast system with zero external dependencies (no Pusher, no Ably).
//!
//! ## Architecture
//!
//! ```text
//! Browser (JS)  ←── WebSocket ──→  Axum server
//!      │                               │
//!      │  {"type":"subscribe",          │  BroadcastMessage { event, data, channels }
//!      │   "channel":"chat"}            │       ↓
//!      │                                │  NativeBroadcaster
//!      │  {"event":"new-msg",           │       ↓
//!      │   "data":{...}}               │  SubscriberRegistry → WebSocket clients
//! ```
//!
//! ## Routes
//!
//! | Method | URI          | Description                     |
//! |--------|--------------|---------------------------------|
//! | GET    | `/`          | Dashboard with WebSocket client |
//! | GET    | `/ws`        | WebSocket upgrade endpoint      |
//! | POST   | `/broadcast` | Send a broadcast event          |
//!
//! ## Usage
//!
//! ```bash
//! # Terminal 1: start the server
//! cargo run --example websocket_broadcast
//!
//! # Terminal 2: send a broadcast
//! curl -X POST http://localhost:8080/broadcast \
//!   -H "Content-Type: application/json" \
//!   -d '{"channel":"chat","event":"new-message","data":{"text":"Hello!"}}'
//!
//! # Open http://localhost:8080 in a browser to see the WebSocket client
//! # in action, or use a WebSocket CLI like websocat:
//! # websocat ws://localhost:8080/ws
//! ```
//!
//! ## Client Protocol
//!
//! The WebSocket client sends JSON control messages:
//!
//! ```json
//! {"type": "subscribe", "channel": "chat"}
//! {"type": "unsubscribe", "channel": "chat"}
//! {"type": "ping"}
//! ```
//!
//! The server responds with:
//!
//! ```json
//! {"type": "subscribed", "channel": "chat"}
//! {"type": "unsubscribed", "channel": "chat"}
//! {"type": "pong"}
//! ```
//!
//! Broadcast events are pushed as:
//!
//! ```json
//! {"event": "new-message", "data": {"text": "Hello!"}, "channel": "chat"}
//! ```

use std::sync::Arc;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::{get, post},
    Extension, Router,
};
use larastvel_core::broadcasting::{
    ws_handler, BroadcastMessage, Broadcaster, NativeBroadcaster, SubscriberRegistry,
};
use larastvel_core::serde::{Deserialize, Serialize};
use larastvel_core::serde_json::json;
use tokio::sync::Mutex;

// =============================================================================
// SHARED STATE
// =============================================================================

/// Application state shared across handlers via Axum's State extractor.
#[derive(Clone)]
struct AppState {
    registry: SubscriberRegistry,
    broadcast_log: Arc<Mutex<Vec<BroadcastLogEntry>>>,
}

/// An entry in the broadcast event log.
#[derive(Debug, Clone, Serialize)]
struct BroadcastLogEntry {
    id: u64,
    channel: String,
    event: String,
    data: String,
    sent_at: String,
}

// =============================================================================
// MAIN
// =============================================================================

#[tokio::main]
async fn main() {
    let state = AppState {
        registry: SubscriberRegistry::new(),
        broadcast_log: Arc::new(Mutex::new(Vec::new())),
    };

    let app = Router::new()
        .route("/", get(dashboard))
        .route("/ws", get(ws_handler))
        .route("/broadcast", post(broadcast_event))
        .route("/broadcast/log", get(broadcast_log_list))
        .layer(Extension(state.registry.clone()))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind port 8080");

    println!("WebSocket broadcast server running on http://localhost:8080");
    println!("WebSocket endpoint: ws://localhost:8080/ws");

    axum::serve(listener, app).await.unwrap();
}

// =============================================================================
// HANDLERS
// =============================================================================

/// Serve the dashboard HTML page with a built-in WebSocket client.
async fn dashboard(State(state): State<AppState>) -> Html<String> {
    let log = state.broadcast_log.lock().await;
    let log_entries: String = log
        .iter()
        .rev()
        .take(20)
        .map(|e| {
            format!(
                r#"<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>"#,
                e.id,
                html_escape(&e.channel),
                html_escape(&e.event),
                html_escape(&truncate(&e.data, 50)),
                e.sent_at,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let log_section = if log.is_empty() {
        r#"<tr><td colspan="5" class="empty">No events broadcast yet</td></tr>"#.to_string()
    } else {
        log_entries
    };

    Html(format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width,initial-scale=1.0">
<title>Larastvel WebSocket Broadcast</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;background:linear-gradient(135deg,#0f172a 0%,#1e293b 100%);min-height:100vh;padding:2rem;color:#e2e8f0}}
.container{{max-width:960px;margin:0 auto}}
h1{{font-size:2rem;font-weight:800;margin-bottom:0.25rem;background:linear-gradient(135deg,#6366f1,#8b5cf6,#ec4899);-webkit-background-clip:text;-webkit-text-fill-color:transparent}}
.subtitle{{color:#94a3b8;margin-bottom:2rem;font-size:1rem}}
.stats{{display:flex;gap:1rem;margin-bottom:2rem}}
.stat{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:8px;padding:1rem 1.5rem;flex:1}}
.stat-value{{font-size:1.75rem;font-weight:700;color:#f1f5f9}}
.stat-label{{font-size:0.75rem;color:#64748b;text-transform:uppercase;letter-spacing:0.05em}}
.card{{background:rgba(255,255,255,0.05);border:1px solid rgba(255,255,255,0.1);border-radius:12px;padding:1.5rem;backdrop-filter:blur(10px);margin-bottom:1.5rem}}
.card h2{{font-size:1.125rem;margin-bottom:1rem;color:#f1f5f9}}
#status{{padding:0.75rem;border-radius:6px;margin-bottom:1rem;font-size:0.875rem;font-family:monospace}}
#status.connected{{background:#064e3b;color:#6ee7b7}}
#status.disconnected{{background:#7f1d1d;color:#fca5a5}}
#messages{{background:rgba(0,0,0,0.3);border-radius:8px;padding:1rem;max-height:300px;overflow-y:auto;font-family:monospace;font-size:0.8125rem;margin-bottom:1rem}}
#messages .msg{{padding:0.5rem;border-bottom:1px solid #1e293b;color:#e2e8f0}}
#messages .msg:last-child{{border-bottom:none}}
#messages .msg .ts{{color:#64748b;margin-right:0.5rem}}
#messages .msg .event{{color:#a5b4fc;font-weight:600}}
#messages .msg .data{{color:#94a3b8}}
label{{display:block;color:#cbd5e1;font-size:0.8125rem;font-weight:600;margin-bottom:0.25rem}}
input{{width:100%;padding:0.625rem;border:1px solid #475569;border-radius:6px;background:#0f172a;color:#e2e8f0;font-size:0.875rem;outline:none;transition:border-color 0.2s;margin-bottom:0.75rem}}
input:focus{{border-color:#6366f1}}
.btn{{padding:0.5rem 1rem;background:#6366f1;color:#fff;border:none;border-radius:6px;font-size:0.8125rem;font-weight:600;cursor:pointer;transition:all 0.2s}}
.btn:hover{{background:#4f46e5;transform:translateY(-1px)}}
.btn.danger{{background:#ef4444}}
.btn.danger:hover{{background:#dc2626}}
table{{width:100%;border-collapse:collapse;font-size:0.8125rem}}
th{{text-align:left;color:#94a3b8;font-size:0.75rem;text-transform:uppercase;letter-spacing:0.05em;padding:0.625rem 0.5rem;border-bottom:1px solid #334155}}
td{{padding:0.5rem;border-bottom:1px solid #1e293b;font-family:monospace}}
.empty{{text-align:center;color:#64748b;padding:1.5rem;font-size:0.875rem}}
.back{{color:#6366f1;text-decoration:none;font-size:0.8125rem;display:block;margin-bottom:1rem}}
.back:hover{{text-decoration:underline}}
</style>
</head>
<body>
<div class="container">
<a href="/" class="back">← Home</a>
<h1>📡 WebSocket Broadcast</h1>
<p class="subtitle">Self-hosted real-time events via <code>NativeBroadcaster</code> + <code>ws_handler</code></p>

<div class="stats">
<div class="stat"><div class="stat-value" id="ws-status">Disconnected</div><div class="stat-label">WebSocket</div></div>
<div class="stat"><div class="stat-value" id="channel-count">0</div><div class="stat-label">Channels</div></div>
<div class="stat"><div class="stat-value" id="event-count">{}</div><div class="stat-label">Events</div></div>
</div>

<div class="card">
<h2>🔌 WebSocket Client</h2>
<div id="status" class="disconnected">⚪ Disconnected</div>
<div id="messages"><div class="msg" style="color:#64748b">Waiting for connection...</div></div>
<div style="display:flex;gap:0.5rem">
<button class="btn" onclick="connect()">Connect</button>
<button class="btn danger" onclick="disconnect()">Disconnect</button>
<button class="btn" onclick="sendPing()">Ping</button>
</div>
</div>

<div class="card">
<h2>📨 Broadcast Form</h2>
<form id="broadcast-form">
<label>Channel</label><input type="text" id="channel" value="chat" placeholder="channel name">
<label>Event</label><input type="text" id="event" value="new-message" placeholder="event name">
<label>Data (JSON)</label><input type="text" id="data" value='{{"text":"Hello from Larastvel!"}}' placeholder='{{"key":"value"}}'>
<button type="submit" class="btn">Broadcast Event</button>
</form>
</div>

<div class="card">
<h2>📋 Event Log</h2>
<table><thead><tr><th>ID</th><th>Channel</th><th>Event</th><th>Data</th><th>Sent</th></tr></thead>
<tbody>{}</tbody></table>
</div>
</div>

<script>
let ws = null;
const messages = document.getElementById('messages');
const status = document.getElementById('status');
const wsStatus = document.getElementById('ws-status');

function log(type, text) {{
    const div = document.createElement('div');
    div.className = 'msg';
    const ts = new Date().toLocaleTimeString();
    if (type === 'event') {{
        div.innerHTML = `<span class="ts">${{ts}}</span><span class="event">📡 ${{text}}</span>`;
    }} else if (type === 'sent') {{
        div.innerHTML = `<span class="ts">${{ts}}</span><span class="data">▲ ${{text}}</span>`;
    }} else if (type === 'received') {{
        div.innerHTML = `<span class="ts">${{ts}}</span><span class="data">▼ ${{text}}</span>`;
    }} else {{
        div.innerHTML = `<span class="ts">${{ts}}</span><span style="color:#${{type === 'error' ? 'fca5a5' : '94a3b8'}}">${{text}}</span>`;
    }}
    messages.appendChild(div);
    messages.scrollTop = messages.scrollHeight;
}}

function connect() {{
    if (ws && ws.readyState === WebSocket.OPEN) return;
    ws = new WebSocket('ws://' + window.location.host + '/ws');
    ws.onopen = () => {{
        status.className = 'connected';
        status.textContent = '🟢 Connected';
        wsStatus.textContent = 'Connected';
        log('info', 'WebSocket connected');
        ws.send(JSON.stringify({{type: 'subscribe', channel: 'chat'}}));
    }};
    ws.onclose = () => {{
        status.className = 'disconnected';
        status.textContent = '🔴 Disconnected';
        wsStatus.textContent = 'Disconnected';
        log('info', 'WebSocket disconnected');
        ws = null;
    }};
    ws.onerror = () => {{
        log('error', 'WebSocket error');
    }};
    ws.onmessage = (event) => {{
        try {{
            const data = JSON.parse(event.data);
            if (data.type === 'subscribed') {{
                log('received', 'Subscribed to "' + data.channel + '"');
            }} else if (data.type === 'pong') {{
                log('received', 'Pong');
            }} else if (data.event) {{
                log('event', JSON.stringify(data));
            }} else {{
                log('received', event.data);
            }}
        }} catch (e) {{
            log('received', event.data);
        }}
    }};
}}

function disconnect() {{
    if (ws) {{
        ws.close();
        ws = null;
    }}
}}

function sendPing() {{
    if (ws && ws.readyState === WebSocket.OPEN) {{
        ws.send(JSON.stringify({{type: 'ping'}}));
        log('sent', 'Ping');
    }}
}}

document.getElementById('broadcast-form').addEventListener('submit', async (e) => {{
    e.preventDefault();
    const channel = document.getElementById('channel').value;
    const event = document.getElementById('event').value;
    let data;
    try {{
        data = JSON.parse(document.getElementById('data').value);
    }} catch (err) {{
        data = {{text: document.getElementById('data').value}};
    }}
    try {{
        const res = await fetch('/broadcast', {{
            method: 'POST',
            headers: {{'Content-Type': 'application/json'}},
            body: JSON.stringify({{channel, event, data}}),
        }});
        const result = await res.json();
        log(res.ok ? 'sent' : 'error', 'Broadcast: ' + JSON.stringify(result));
        if (res.ok) setTimeout(() => location.reload(), 500);
    }} catch (err) {{
        log('error', 'Broadcast failed: ' + err);
    }}
}});

connect();
</script>
</body>
</html>"#,
        log.len(),
        log_section,
    ))
}

/// POST /broadcast — broadcast an event to all WebSocket clients.
async fn broadcast_event(
    State(state): State<AppState>,
    axum::Json(body): axum::Json<BroadcastRequest>,
) -> impl IntoResponse {
    let broadcaster = NativeBroadcaster::new("native", state.registry.clone());

    let message = BroadcastMessage::new(&body.event, body.data.clone(), vec![body.channel.clone()]);

    match broadcaster.broadcast(message).await {
        Ok(()) => {
            let mut log = state.broadcast_log.lock().await;
            let id = log.len() as u64 + 1;
            let now = chrono_now();
            log.push(BroadcastLogEntry {
                id,
                channel: body.channel.clone(),
                event: body.event.clone(),
                data: body.data.to_string(),
                sent_at: now,
            });
            (
                StatusCode::OK,
                Json(json!({"status":"ok","id":id,"channel":body.channel,"event":body.event})),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error":e.to_string()})),
        )
            .into_response(),
    }
}

/// GET /broadcast/log — return the event log as JSON.
async fn broadcast_log_list(State(state): State<AppState>) -> Json<Vec<BroadcastLogEntry>> {
    let log = state.broadcast_log.lock().await;
    Json(log.clone())
}

// =============================================================================
// HELPERS
// =============================================================================

fn chrono_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let secs = now % 60;
    let mins = (now / 60) % 60;
    let hours = (now / 3600) % 24;
    format!("{:02}:{:02}:{:02}", hours, mins, secs)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s.to_string()
    }
}

// =============================================================================
// REQUEST TYPES
// =============================================================================

#[derive(Debug, Deserialize)]
struct BroadcastRequest {
    channel: String,
    event: String,
    data: serde_json::Value,
}
