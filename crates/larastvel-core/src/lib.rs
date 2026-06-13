extern crate self as larastvel_core;

pub mod auth;
pub mod broadcasting;
pub mod cache;
pub mod config;
pub mod console;
pub mod database;
pub mod encryption;
pub mod events;
pub mod foundation;
pub mod hash;
pub mod http;
pub mod logging;
pub mod mail;
pub mod middleware;
pub mod models;
pub mod notifications;
pub mod pagination;
pub mod queue;
pub mod rate_limiter;
pub mod routing;
pub mod scheduling;
pub mod session;
pub mod sms;
pub mod storage;
pub mod support;
pub mod translation;
pub mod validation;
pub mod view;

pub use axum;
pub use sea_orm;
pub use sea_orm_migration;
pub use serde;
pub use serde_json;
pub use tokio;

pub use auth::{
    authorize, check_ability, require_ability, require_verified_email, Auth, AuthError,
    AuthenticatedUser, Claims, EmailVerificationBroker, EmailVerificationError, Gate, GateCheck,
    PasswordResetBroker, PasswordResetConfig, PasswordResetError, PasswordResetToken, Policy,
    VerificationChecker, VerifiedUser,
};
pub use broadcasting::{
    BroadcastError, BroadcastEvent, BroadcastManager, BroadcastMessage, Broadcaster, Channel,
    PresenceChannelData,
};
pub use cache::{
    prefixed_key, CacheError, CacheItem, CacheManager, CacheStore, DEFAULT_TTL_SECONDS, FOREVER_TTL,
};
pub use config::Config;
pub use console::ConsoleKernel;
pub use database::{DatabaseManager, DatabaseSeeder, Seeder};
pub use events::EventService;
pub use foundation::{Application, EventServiceProvider, Kernel, RouteServiceProvider, ServiceProvider};
pub use http::Request;
pub use larastvel_macros::{controller, route, Resource};
pub use notifications::{
    BroadcastPayload, DatabaseNotification, Notifiable, Notification, NotificationChannel,
    NotificationError, NotificationSender,
};
pub use queue::{
    dispatch, DatabaseQueue, InMemoryQueue, JobResolver, Queue, QueueManager, QueueWorker,
    ShouldQueue, SyncQueue,
};
pub use rate_limiter::{
    rate_limit_middleware, rate_limiter, RateLimitConfig, RateLimitExceeded, RateLimiter,
    RateLimiterRegistry,
};
pub use routing::Registrar;
pub use scheduling::{parse_cron, CronExpression, Schedule, ScheduleManager, ScheduledEvent};
pub use translation::{
    __with, has_translation, load_translation_directory, load_translation_file,
    load_translation_json, locale, set_fallback_locale, set_locale, trans_choice,
    trans_choice_with, Translator, __,
};
