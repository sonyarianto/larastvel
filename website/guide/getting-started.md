# Quick Start

## Installation

```bash
git clone https://github.com/sonyarianto/larastvel.git
cd larastvel
cargo run
```

Or scaffold a new project:

```bash
git clone https://github.com/sonyarianto/larastvel.git
cd larastvel
cargo run -p larastvel-cli -- new my-app
cd ../my-app
cargo run
```

Visit **http://localhost:8080** — you're up!

## Project Structure

```
my-app/
├── config/
│   ├── app.toml
│   ├── database.toml
│   ├── logging.toml
│   └── view.toml
├── src/
│   ├── routes/
│   │   ├── web.rs
│   │   └── api.rs
│   ├── models/
│   ├── database/
│   │   └── migrations/
│   └── main.rs
├── resources/
│   └── views/
├── public/
├── storage/
└── Cargo.toml
```

## Your First Route

```rust
// src/routes/web.rs
pub fn web(router: &Registrar) {
    router.get("/", || async {
        axum::response::Html("<h1>Welcome to Larastvel</h1>")
    });
}
```

## Your First Model

```rust
// src/models/user.rs
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i32,
    pub name: String,
    pub email: String,
}
```

## Next Steps

- Learn about [Configuration](/guide/configuration)
- Understand the [Architecture](/guide/architecture)
- Explore [Routing](/guide/routing) in depth
