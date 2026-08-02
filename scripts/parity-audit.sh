#!/usr/bin/env bash
#
# Larastvel ↔ Laravel 13 parity audit.
#
# Symbol-level drift detector: probes the workspace for the public API
# surface that must exist to claim Laravel-13 feature parity. A red probe
# means the feature is missing or was renamed/removed — either a gap to
# close or a regression to fix.
#
# Usage:
#   bash scripts/parity-audit.sh             # informational report, exit 0
#   bash scripts/parity-audit.sh --strict    # exit 1 if any non-deferred gap
#
# The report is a markdown table with one row per feature and the first
# matching file:line as evidence.
#
# Keeping this script honest:
#   - Every Laravel feature implemented must get a probe row here.
#   - Every probe must match a REAL symbol in the codebase (grep first).
#   - Deferred items (acknowledged gaps tracked in PARITY.md) go in
#     DEFERRED_FEATURES below; everything else must stay green.
#   - Run `bash scripts/parity-audit.sh --strict` before every release
#     (see AGENTS.md -> Parity Audit & Drift Prevention).

set -u

STRICT=0
for arg in "$@"; do
  case "$arg" in
    --strict) STRICT=1 ;;
    *)
      echo "Unknown option: $arg (expected: --strict)" >&2
      exit 2
      ;;
  esac
done

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CORE="$ROOT/crates/larastvel-core/src"
MACROS="$ROOT/crates/larastvel-macros/src"
CLI="$ROOT/crates/larastvel-cli/src"
TESTING="$ROOT/crates/larastvel-testing/src"
TINKER="$ROOT/crates/larastvel-tinker/src"
NEW="$ROOT/crates/larastvel-new/src"

# Feature name | extended-regex probe | search path.
# The probe is matched against *.rs files with grep -rEn. Evidence is the
# first match, reported as file:line.
FEATURES="
Database connections & pooling|pub struct DatabaseManager|$CORE/database
Migrations (migrate/fresh/rollback/status)|pub async fn migrate|$CORE/database
Database seeders|pub trait Seeder|$CORE/database
DB transactions (transaction/begin/commit/rollback)|pub async fn transaction|$CORE/database
Model CRUD (find/all/insert/update/delete)|pub trait DbModel|$CORE/models
Soft deletes|pub trait SoftDeletes|$CORE/models
Auto timestamps (created_at/updated_at)|pub trait Timestamps|$CORE/models
Relationships (has_many/belongs_to)|fn has_many|$CORE/models
Eager loading (load_many/load_one)|fn load_many|$CORE/models
Global query scopes|fn scope_filter|$CORE/models
Model lifecycle events (created/updated/deleted)|ModelCreated<|$CORE/events
Model observers (#[observer])|fn observer|$MACROS
Queue manager + job routing|pub struct QueueManager|$CORE/queue
Queue worker (tries/backoff/timeout)|DEFAULT_MAX_ATTEMPTS|$CORE/queue
Job delay (#[job(delay)]/ShouldQueue::delay_seconds)|fn delay_seconds|$CORE/queue
Failed job storage (failed_jobs)|failed_jobs|$CORE/queue
Job batches (Bus::batch)|pub struct JobBatch|$CORE/queue
Validator framework|pub struct Validator|$CORE/validation
Validation rules (built-in set)|fn check_rule|$CORE/validation
Validation unique/exists DB rules|Rule::Unique|$CORE/validation
Validation base64 rule|Rule::Base64|$CORE/validation
HTTP routing (get/post/put/patch/delete)|pub fn post<|$CORE/routing
Route groups / view / websocket|pub fn group|$CORE/routing
Middleware aliases + presets|pub fn with_middleware|$CORE/routing
Resource controllers|pub trait ResourceController|$CORE/routing
Route model binding (implicit {model})|ModelPath|$CORE/routing
Route conflict detection (route:conflicts)|route_conflicts|$CORE/routing
Route metadata (->metadata())|route_with_metadata|$CORE/routing
Signed URLs (signedRoute)|pub fn signed_route|$CORE/routing
Route/controller/validate macros|pub fn route\(|$MACROS
Auth (JWT guards)|pub struct Auth|$CORE/auth
Passkey authentication (WebAuthn)|pub struct PasskeyService|$CORE/auth
Policies & Gate|struct Gate|$CORE/auth
Sessions|pub struct Session|$CORE/session
CSRF protection|csrf|$CORE/session
Caching (file/redis stores)|pub struct CacheManager|$CORE/cache
Redis cache store|pub struct RedisStore|$CORE/cache
Cache locks (Lock/with_lock/get_locked)|pub struct Lock|$CORE/cache
Str helpers (slug/studly/camel/snake)|pub struct Str|$CORE/support
Stringable|struct Stringable|$CORE/support
Collection (map/filter/pluck/reduce)|pub struct Collection|$CORE/support
LazyCollection|pub struct LazyCollection|$CORE/support
Concurrency helpers (concurrent)|pub async fn concurrent|$CORE/concurrency
Process facade (run/foreground/timeout)|pub struct ProcessBuilder|$CORE/process
Faker (name/email/word/uuid/...)|pub struct Faker|$CORE/models
Http client (Http facade)|pub struct Http|$CORE/support
Path helpers (base_path/storage_path/...)|pub fn base_path|$CORE/support
Redirect/back/abort helpers|pub fn abort_if|$CORE/support/helpers.rs
Mail (to/cc/bcc/attachments)|pub struct Mailable|$CORE/mail
Notifications|pub struct NotificationSender|$CORE/notifications
Events & listeners|pub trait Listener|$CORE/events
Broadcasting (websocket)|pub struct BroadcastMessage|$CORE/broadcasting
Reverb DB scaling driver|pub struct ReverbDatabaseBroadcaster|$CORE/broadcasting
Scheduling|pub struct ScheduleManager|$CORE/scheduling
Rate limiting|pub struct RateLimiter|$CORE/rate_limiter
Pagination|pub struct Paginator|$CORE/pagination
Pipeline|pub struct Pipeline|$CORE/pipeline
Storage (filesystems)|pub struct StorageManager|$CORE/storage
Translation|pub struct Translator|$CORE/translation
View rendering|pub struct ViewFactory|$CORE/view
Blade components & slots (x-slot/@slot)|extract_slots|$CORE/view
AI chat SDK (chat/generate)|pub struct Ai|$CORE/ai
AI agents (tool calling)|pub struct Agent|$CORE/ai
AI media (image/tts/stt/moderation)|ImageOptions|$CORE/ai
AI reranking|RerankOptions|$CORE/ai
Vector stores (file/pgvector)|FileVectorStore|$CORE/ai
Encryption (AES)|pub struct Encrypter|$CORE/encryption
Hashing (bcrypt/argon2)|pub fn make|$CORE/hash
Logging|pub fn init|$CORE/logging
Monthly log driver (laravel-YYYY-MM.log)|pub struct MonthlyWriter|$CORE/logging
Console kernel + commands|pub trait Command|$CORE/console
Maintenance mode (down/up)|maintenance_down|$CLI
CLI queue commands (failed/retry/flush)|queue:failed|$CLI
CLI about/optimize/config:show|config:show|$CLI
CLI make generators|make_migration|$CLI
TestClient/RefreshDatabase|pub struct TestClient|$TESTING
Tinker REPL|Tinker|$TINKER
Scaffolding (larastvel new)|create_project|$NEW
"

# Features documented as known gaps in PARITY.md. A red probe here is
# expected (acknowledged drift) and does NOT fail a --strict run.
# Keep this list in sync with PARITY.md's "tracked gaps" section.
DEFERRED_FEATURES="
First-party image processing (Image/ImageManager, resize/cover/crop)|pub struct ImageManager|$CORE
Container attribute #[BindWhen] (conditional bindings)|BindWhen|$CORE
"

is_deferred() {
  local name="$1"
  echo "$DEFERRED_FEATURES" | grep -qF "$name|"
}

report=""
red_count=0
green_count=0

while IFS='|' read -r name probe path; do
  [ -z "$name" ] && continue
  match="$(grep -rEn --include='*.rs' --max-count=1 "$probe" "$path" 2>/dev/null | head -1)"
  if [ -n "$match" ]; then
    green_count=$((green_count + 1))
    file="${match%%:*}"
    line="$(echo "$match" | cut -d: -f2)"
    status="OK"
    evidence="${file#${ROOT}/}:${line}"
  else
    red_count=$((red_count + 1))
    status="GAP"
    evidence="not found in ${path#${ROOT}/}"
  fi
  report="$report| $name | $status | \`$evidence\` |\n"
done <<< "$FEATURES"

while IFS='|' read -r name probe path; do
  [ -z "$name" ] && continue
  match="$(grep -rEn --include='*.rs' --max-count=1 "$probe" "$path" 2>/dev/null | head -1)"
  if [ -n "$match" ]; then
    green_count=$((green_count + 1))
    file="${match%%:*}"
    line="$(echo "$match" | cut -d: -f2)"
    status="OK (deferred, now implemented)"
    evidence="${file#${ROOT}/}:${line}"
  else
    red_count=$((red_count + 1))
    status="GAP (deferred)"
    evidence="tracked in PARITY.md"
  fi
  report="$report| $name | $status | \`$evidence\` |\n"
done <<< "$DEFERRED_FEATURES"

date_utc="$(date -u +%Y-%m-%dT%H:%MZ)"
echo "## Larastvel ↔ Laravel 13 parity audit — $date_utc"
echo ""
echo "| Feature | Status | Evidence |"
echo "|---------|--------|----------|"
printf "%b" "$report"
echo ""
echo "**Summary:** $green_count implemented, $red_count gaps (including deferred)."

if [ "$STRICT" = "1" ]; then
  undeferred_gaps="$(printf "%b" "$report" | grep 'GAP' | grep -v 'deferred' | grep -v 'GAP (deferred)' || true)"
  if [ -n "$undeferred_gaps" ]; then
    echo ""
    echo "STRICT MODE FAILED: unreported gaps found (features not in PARITY.md's deferred list):"
    printf "%s\n" "$undeferred_gaps"
    exit 1
  fi
  echo ""
  echo "Strict mode: PASS — all non-deferred probes green."
fi

exit 0
