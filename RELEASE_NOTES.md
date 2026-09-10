# Release Notes

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
