# Deployment

## Building for Production

```bash
cargo build --release
```

## Environment Configuration

Set the `APP_ENV` environment variable:

```bash
export APP_ENV=production
```

And ensure `config/app.toml` has:

```toml
env = "production"
debug = false
```

## Database

```bash
larastvel migrate
```

## Running

```bash
./target/release/larastvel serve
```

## Vercel

The website can be deployed to Vercel. A `vercel.json` is included in the `website/` directory.

## Docker

Example Dockerfile:

```dockerfile
FROM rust:1-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/target/release/larastvel /app/larastvel
COPY --from=builder /app/config /app/config
COPY --from=builder /app/resources /app/resources
EXPOSE 8080
CMD ["./larastvel", "serve"]
```
