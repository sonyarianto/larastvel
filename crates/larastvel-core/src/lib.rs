extern crate self as larastvel_core;

pub mod auth;
pub mod bootstrap;
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
pub use bootstrap::App;
pub use broadcasting::{
    BroadcastError, BroadcastEvent, BroadcastManager, BroadcastMessage, Broadcaster, Channel,
    NativeBroadcaster, PresenceChannelData, SubscriberRegistry,
};
pub use cache::{
    prefixed_key, CacheError, CacheItem, CacheManager, CacheStore, DEFAULT_TTL_SECONDS, FOREVER_TTL,
};
pub use config::Config;
pub use console::{Command, ConsoleKernel};
pub use database::{DatabaseManager, DatabaseSeeder, Seeder};
pub use encryption::{generate_key, EncryptError, Encrypter};
pub use events::EventService;
pub use foundation::{
    Application, DeferrableProvider, EventServiceProvider, Kernel, RouteServiceProvider,
    ServiceProvider,
};
pub use hash::{check as hash_check, is_hashed, make as hash_make, needs_rehash, HashError};
pub use http::{Error as HttpError, JsonResponse, LarastvelResult, Request};
pub use larastvel_macros::{controller, delete, get, patch, post, put, route, ws, Resource};
pub use logging::init as logging_init;
pub use middleware::{cors_middleware, request_logger};
pub use models::factory::{Faker, ModelFactory};
pub use models::serialization::{ApiResource, JsonResource, ResourceCollection, SerializesToArray};
pub use notifications::{
    BroadcastPayload, DatabaseNotification, Notifiable, Notification, NotificationChannel,
    NotificationError, NotificationSender,
};
pub use pagination::{paginate, PaginationParams, Paginator};
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
pub use session::{Session, SessionHandle};
pub use sms::{LogSmsSender, SmsError, SmsMessage, SmsSender, VonageSmsSender};
pub use storage::{Filesystem, LocalDisk, StorageError, StorageManager};
pub use support::{
    app_path, base_path, collect_items, config_path, now, public_path, resource_path, storage_path,
    today, Arr, Collection, Dt, Http, Number, PendingRequest, Prompt, Str, Stringable, Vite,
};
pub use translation::{
    __with, has_translation, load_translation_directory, load_translation_file,
    load_translation_json, locale, set_fallback_locale, set_locale, trans_choice,
    trans_choice_with, Translator, __,
};
pub use validation::{validate, ValidatedJson, ValidatedQuery, ValidationErrors, Validator};
pub use view::{ViewError, ViewFactory};
