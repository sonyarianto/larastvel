# Deployment

## Building for Production

```bash
cargo build --release
```

The release binary is produced at `target/release/larastvel`. For smaller
binaries, enable LTO and strip symbols in `Cargo.toml`:

```toml
[profile.release]
lto = true
strip = true
```

## Environment Configuration

Set `APP_ENV=production` and provide a strong `APP_KEY`:

```bash
export APP_ENV=production
export APP_KEY=...
```

Generate an application key with the CLI:

```bash
larastvel key:generate
```

Ensure `config/app.toml` has:

```toml
env = "production"
debug = false
```

Secrets (database credentials, API keys) should come from environment
variables or your host's secret manager, not the committed `config/*.toml`
files.

## Database

```bash
larastvel migrate
```

Run migrations as part of your deploy pipeline before rolling out the new
binary.

## Running

```bash
./target/release/larastvel serve
```

For production, run it as a service. Example systemd unit
(`/etc/systemd/system/larastvel.service`):

```ini
[Unit]
Description=Larastvel application
After=network.target

[Service]
Type=simple
User=www-data
WorkingDirectory=/var/www/my-app
ExecStart=/var/www/my-app/larastvel serve
Restart=on-failure
Environment=APP_ENV=production
Environment=APP_KEY=...

[Install]
WantedBy=multi-user.target
```

Then:

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now larastvel
```

## Reverse Proxy

Run Larastvel behind a reverse proxy for TLS. Example nginx site:

```nginx
server {
    listen 443 ssl;
    server_name example.com;

    ssl_certificate     /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

The `Upgrade`/`Connection` headers keep WebSocket broadcasting working
through the proxy.

## Docker

Example multi-stage Dockerfile:

```dockerfile
FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release --locked

FROM debian:bookworm-slim
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/larastvel /app/larastvel
COPY --from=builder /app/config /app/config
COPY --from=builder /app/resources /app/resources
EXPOSE 8080
CMD ["./larastvel", "serve"]
```

Run with your configuration mounted in:

```bash
docker build -t my-app .
docker run -d -p 8080:8080 \
  -e APP_ENV=production \
  -e APP_KEY=... \
  -v /etc/my-app/config:/app/config \
  my-app
```

## Documentation Website

The Larastvel docs site itself deploys to Vercel via the `Deploy Docs`
workflow (`.github/workflows/deploy-docs.yml`), with a `vercel.json` in the
`website/` directory. The docs site is hosted at
[larastvel.vercel.app](https://larastvel.vercel.app).
