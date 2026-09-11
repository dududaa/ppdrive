# Release Notes

## v0.1.3 — Bug Fixes

### Bug Fixes

- Fixed `ppdrive serve` failing when run outside the project root — server binary now resolves via `shared::root_dir()` instead of CWD-relative `./server`
- Fixed Postgres startup crash — created `migrations_postgres/` with `BYTEA` (not `BLOB`) and engine-specific migration dispatch
- Fixed SQLite BOOLEAN decode failures — `public` and `EXISTS` queries now decode as `i32` across all engines
- Fixed 500 "Unable To Extract Key!" without a proxy — rate limiter now falls back to the direct `SocketAddr` via `into_make_service_with_connect_info`
- Fixed config parse failure when `static_folders` is omitted — added `#[serde(default)]`

## v0.1.1 — File-Level Privacy & User Auth

### Highlights

- **File-level permissions** — Fine-grained ACLs on private bucket files. Grant read, write, or admin permissions to clients or users.
- **User authentication** — `POST /auth/login` endpoint for email/password login. Users can manage file permissions alongside clients.
- **Permission API** — `POST/DELETE/GET /buckets/{pid}/permissions` for granting, revoking, and listing file permissions.
- **Auto-registration** — Files uploaded to private buckets are automatically registered as assets with admin permissions for the uploader.
- **Asset CLI** — `ppdrive asset grant/revoke/list` commands for managing file permissions from the terminal.
- **User CLI** — `ppdrive user create` command for creating user accounts.
- **AuthExtractor** — Middleware supports both client tokens (`x-ppdrive-client`) and user Bearer tokens (`Authorization: Bearer`).

### Bug Fixes

- Fixed download flow to use proper bucket owner check instead of broken `check_ownership`
- Fixed `list_permissions` queries to resolve grantees from both clients and users tables
- Fixed user `create` to include `updated_at` field

### Documentation

- Added File Permissions API reference
- Added User Authentication API reference
- Added Asset and User CLI command docs
- Added "How It Works" section to homepage with upload/download examples
- Updated Authentication page to cover both client and user auth
- Updated Download page to document file-level permission checks
- Updated Upload page to document private bucket asset auto-registration

### Internal

- Made `validator` always enabled in shared crate (no longer feature-gated)
- Added `hex` dependency for user token signing
- Added `is_admin` field to users migration

## v0.1.0 — First Stable Release

### Highlights

- Per-IP rate limiting (100 req/s, burst 200)
- `POST /buckets` API for client bucket creation
- HTTP metrics at `/metrics` (Prometheus)
- Health check at `/health`
- Graceful shutdown (SIGINT/SIGTERM)
- Configurable DB pool size via `db_pool_size`
- Request timeout (30s, HTTP 408)
- Temp file background cleanup (hourly, 2hr max age)

### Security

- Timing-safe token comparison (Blake3)
- Stable ChaCha20-Poly1305 encryption
- Path traversal protection via `Path::components()`
- No hardcoded database credentials
- Secrets file: 56 bytes, mode `0600`, zeroed on drop
- `#[serde(deny_unknown_fields)]` on all API request structs
- Input validation on all API fields (length, range, format)

### Bug Fixes

- Fixed dollar-sign format string in token parsing
- Fixed download route concurrency limit
- Fixed JSON error responses (no HTML leaking)
- Fixed path resolution (no directory creation on check)
- Fixed bucket size check using `get_folder_size`
- Fixed `AssetOwnerName::from(i16)` silent default
- Fixed `.unwrap()` on `HeaderValue::from_str` in download handler

### Performance

- `spawn_blocking` for path resolution (N+1 query fix)
- SQLite WAL mode enabled
- Configurable connection pool size (`db_pool_size`, default 10)
- Connection pool: 30min max lifetime, 5min idle timeout

### Docker & Deployment

- Multi-stage Dockerfile with non-root user
- `HEALTHCHECK` in Dockerfile
- Docker Compose with PostgreSQL + Redis
- Docker Compose health checks with `service_healthy` conditions
- Fixed port mapping (8000:8000)
- Volume mounts preserve config file

### Install Scripts

- macOS support in `install.sh`
- ARM64 support (Linux, macOS, Windows)
- Version check (skip install if up to date)
- Windows: fixed URLs, User PATH (no admin required)
- Cleanup on failure (not just happy path)

### Platforms

Linux (x86_64, arm64), macOS (x86_64, arm64), Windows (x86_64)
