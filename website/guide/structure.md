# Directory Structure

A typical Larastvel project scaffolded via `larastvel-new`:

```
my-app/
├── Cargo.toml              # Project dependencies
├── config/                 # TOML configuration files
│   ├── app.toml            # Application config
│   ├── database.toml       # Database connection
│   ├── logging.toml        # Logging settings
│   └── view.toml           # Template engine config
├── src/
│   ├── main.rs             # Application entrypoint
│   ├── routes/
│   │   ├── web.rs          # Web routes
│   │   ├── api.rs          # API routes
│   │   └── console.rs      # Scheduled tasks
│   ├── models/             # SeaORM models
│   ├── controllers/        # Request handlers
│   ├── middleware/          # Custom middleware
│   └── database/           # Migrations & seeds
│       └── migrations/
├── resources/
│   ├── views/              # Tera templates
│   ├── lang/               # Translation files
│   ├── css/                # Stylesheets
│   └── js/                 # JavaScript
├── public/                 # Public assets
├── storage/                # Logs, cache, uploaded files
│   ├── app/                # Uploaded files
│   ├── framework/          # Framework cache
│   └── logs/               # Application logs
└── tests/                  # Integration tests
```
