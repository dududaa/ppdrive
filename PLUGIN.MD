# PPDRIVE Plugin Guide

This document describes PPDRIVE's plugin system: what plugins are, how the server invokes them, the officially available plugins and their install commands, the plugin CLI, and a general guide to writing your own plugin.

> **Related documentation**: [Plugin CLI reference](https://ppdrive.dududaa.org/cli/plugin/) · [Upload API (plugin options)](https://ppdrive.dududaa.org/api/upload/) · [Configuration file](https://ppdrive.dududaa.org/configuration/config-file/)

---

## Table of Contents

- [What Plugins Are](#what-plugins-are)
- [How the Server Invokes Plugins](#how-the-server-invokes-plugins)
- [Officially Available Plugins](#officially-available-plugins)
  - [Media Plugins](#media-plugins)
  - [Dashboard](#dashboard)
- [Plugin CLI](#plugin-cli)
- [Writing a Plugin](#writing-a-plugin)
  - [1. Choose an integration point](#1-choose-an-integration-point)
  - [2. Project layout](#2-project-layout)
  - [3. Implement `plugin_dispatch`](#3-implement-plugin_dispatch)
  - [4. Options contract](#4-options-contract)
  - [5. Ids, naming and release assets](#5-ids-naming-and-release-assets)
  - [6. Build and install locally](#6-build-and-install-locally)
  - [7. Publish](#7-publish)
  - [Compatibility](#compatibility)
- [Troubleshooting](#troubleshooting)

---

## What Plugins Are

A PPDRIVE plugin is a single dynamic library (`.so` on Linux, `.dylib` on macOS, `.dll` on Windows) that the server loads at startup to extend its behaviour without recompiling the server itself. Everything beyond the core upload/download/bucket API — the dashboard UI, image conversion and transformation, audio conversion and effects, video conversion and transformation, HLS/DASH packaging — ships as a plugin.

Key properties:

- **Registry + library.** Installed plugins are recorded in `plugins.json` in the PPDRIVE root directory (next to the binaries); their libraries live in `libs/`. The registry entry stores the short id, the library filename, the install source, whether it was built from source, and its activation state.
- **One C entry point.** Every plugin exports exactly one symbol, `plugin_dispatch`, with the C ABI. The server hands it a boxed argument tuple and takes back a boxed `DispatchResponse` (`Ok(payload)` or `Error(message)`). Everything else inside the library is the plugin's own business.
- **Loaded once, kept alive.** The server loads every *active* registry entry at startup and keeps the library handles for its whole lifetime (dropping a handle would unload the `.so` under a running request). Installing, updating or activating a plugin therefore requires a server restart (`ppdrive serve`).
- **Activation flag.** Each entry carries an `active` flag (`ppdrive plugin activate` / `deactivate`). Deactivated plugins stay installed and updatable but are skipped at startup, and requests that need them fail with a message naming the exact activation command.
- **Fail fast, not fail late.** Upload sessions validate their plugin requirements *at session creation* — installed, active, loadable, correct `content_type` family — before a single byte of the file is accepted.
- **Hyphenated ids everywhere.** The stored short id (`image-conversion`), the full id (`ppdrive-image-conversion`), the cargo package (`ppdrive-image-conversion`) and the release asset (`ppdrive-image-conversion-linux-x86_64.so`) are one convention. On input, `ppdrive_`/`_` forms are accepted and normalized.

## How the Server Invokes Plugins

The server knows a fixed set of **integration points**, each bound to a plugin id. A plugin is invoked only if its id matches an integration point — installing a library alone makes it loadable, not called.

| Integration point | Plugin id | Arguments (boxed) | Success payload | When it runs |
|---|---|---|---|---|
| Upload post-processing (media options) | `image-conversion`, `image-transformation`, `audio-conversion`, `audio-effects`, `video-conversion`, `video-transformation` | `(&[u8], &serde_json::Value)` — file bytes + option JSON | `Box<Vec<u8>>` — processed bytes | Inline, before the upload response (default), or in a background task when `background: true` / the matching `*_background` config knob is set |
| Stream packaging | `media-streaming` | `(&Path, &Path, &serde_json::Value)` — input file path, output directory, option JSON | `Box<Vec<u8>>` of UTF-8 JSON: `{"playlist": "...", "files": [...]}` | After the upload is renamed into place (inline or background) |
| Transformed downloads | `image-transformation` | `(&[u8], &serde_json::Value)` — source bytes + option JSON | `Box<Vec<u8>>` — transformed bytes | On the fly, per download request, when a transformation was embedded at sign time or passed as `?image_transformation=` |
| Router (dashboard) | `dashboard` | `Box<AppState>` — the server's axum state | `Box<axum::Router<AppState>>` — routes merged into the app | Once, at startup |

Upload post-processing chains in a fixed order per family — image transform → image convert, audio effects → audio convert, video transform → video convert — then packaging last. The option JSON is the serialized session option (e.g. `video_conversion`), including its `background` field.

Failure semantics:

- **Inline** processing failures fail the operation: the upload returns `500` (`upload failed`), with the plugin's error (e.g. `video-conversion failed: FFmpeg error: …`) logged server-side.
- **Background** processing failures are logged server-side only — the client already received a success response.
- Session-time gate failures are `400` responses with a precise message (see [Troubleshooting](#troubleshooting)).

## Officially Available Plugins

### Media Plugins

All seven media plugins are published as prebuilt libraries (and source) from **[dududaa/ppff](https://github.com/dududaa/ppff)** GitHub releases.

| Plugin id | Session option | What it does |
|---|---|---|
| `image-conversion` | `image_conversion` | Re-encodes images after upload (format, quality, width/height, scale, effort, `max_bytes`) |
| `image-transformation` | `image_transformation` | Typed image operations (crop, rotate, flip, scale, blur, …) on upload **and** on the fly for downloads |
| `audio-conversion` | `audio_conversion` | Transcodes audio after upload (Wav/Mp3/Flac/Aac/Ogg/Opus, quality, sample rate, channels) |
| `audio-effects` | `audio_effects` | Audio effects on upload (volume, fade, speed, EQ, echo, trim, reverse, normalize) |
| `video-conversion` | `video_conversion` | Re-encodes video after upload (10 container/codec targets, quality, scaling, `max_bytes`, fps, …) |
| `video-transformation` | `video_transformation` | Typed video operations (crop, scale, rotate, flip, trim, speed, reverse, …) on upload |
| `media-streaming` | `media_streaming` | Packages uploads into adaptive HLS or DASH (playlist + segments) |

Install any of them with:

```sh
ppdrive plugin add image-conversion      --source github:dududaa/ppff
ppdrive plugin add image-transformation  --source github:dududaa/ppff
ppdrive plugin add audio-conversion      --source github:dududaa/ppff
ppdrive plugin add audio-effects         --source github:dududaa/ppff
ppdrive plugin add video-conversion      --source github:dududaa/ppff
ppdrive plugin add video-transformation  --source github:dududaa/ppff
ppdrive plugin add media-streaming       --source github:dududaa/ppff
```

Notes:

- By default the matching prebuilt asset (`ppdrive-<id>-<os>-<arch>.<ext>`) is downloaded from the repository's **latest** release. Pin a release with `--version` — the version **without** the `v` prefix (e.g. `--version 0.3.0` resolves tag `v0.3.0`).
- Prebuilt assets cover **linux-x86_64, macos-x86_64, macos-aarch64 and windows-x86_64**. On other platforms (e.g. linux-aarch64) add `--build` to compile from the release source archive instead.
- The media plugins need the FFmpeg shared libraries (libav*) present on the host.
- Underscore ids work too: `ppdrive plugin add image_conversion …` normalizes to `image-conversion`.

### Dashboard

The **dashboard UI** plugin ([dududaa/ppdrive-dashboard](https://github.com/dududaa/ppdrive-dashboard)) is a *router* plugin: at startup the server dispatches its axum `AppState` to it and merges the returned `Router` into the app (UI assets, auth, dashboard API routes). It has no prebuilt releases — build it from source. The router crate depends on a **sibling checkout of this repository**, so clone them side by side:

```sh
git clone https://github.com/dududaa/ppdrive ~/src/ppdrive
git clone https://github.com/dududaa/ppdrive-dashboard ~/src/ppdrive-dashboard

ppdrive plugin add dashboard --source ~/src/ppdrive-dashboard/router --local --build
ppdrive serve
```

A prebuilt library can be installed directly: `ppdrive plugin add dashboard --source ./ppdrive-dashboard-linux-x86_64.so --local`.

## Plugin CLI

All plugin management lives under `ppdrive plugin`. It works without a database or config file.

```sh
# Install (the ppdrive- prefix on <ID> is optional; --source is where to install from)
ppdrive plugin add <ID> --source <SOURCE> [--version <TAG>] [--local] [--build]

# Activation (persisted; takes effect on the next server start)
ppdrive plugin activate <ID>
ppdrive plugin deactivate <ID>

# Inspect
ppdrive plugin list

# Remove (deletes the registry entry and libs/<library>)
ppdrive plugin remove <ID>

# Update one plugin, or all when <ID> is omitted
ppdrive plugin update [<ID>]
```

| Flag | Meaning |
|---|---|
| `--source` | `github:owner/repo` / `owner/repo` / `https://github.com/owner/repo`, a `.git` URL, or (with `--local`) a local library file or source directory |
| `--version` | Release to install from (default `latest`) |
| `--local` | Install from a local file or local source directory |
| `--build` | Compile from source with `cargo build --release --lib --package ppdrive-<id>` instead of downloading a release asset |

`update` re-fetches from the recorded source: remote plugins reinstall from the latest release (built when installed with `--build`, asset otherwise); local plugins rebuild from their recorded directory. Updates preserve the activation state.

**Restart the server (`ppdrive serve`) after adding, updating, removing, activating or deactivating a plugin** — plugins are loaded only at startup.

## Writing a Plugin

The reference implementation of every pattern below lives in **[dududaa/ppff](https://github.com/dududaa/ppff)** (media plugins) and **[dududaa/ppdrive-dashboard](https://github.com/dududaa/ppdrive-dashboard)** (router plugin).

### 1. Choose an integration point

The server dispatches to **fixed plugin ids** (see [the table](#how-the-server-invokes-plugins)). A new library is only invoked when it occupies one of those slots:

- **Replacement / alternative implementation** — implement an existing id (e.g. your own `image-conversion` or `media-streaming`) with the same argument tuple, payload type and options contract. This is how you swap FFmpeg for your own engine, or ship a hardened build.
- **Router slot** — the id `dashboard` receives `AppState` and returns a `Router<AppState>` merged into the app. This is the only way to add new HTTP routes from a plugin today.
- **New integration point** — wiring a brand-new session option or hook requires a change to the server itself; open a PR against [dududaa/ppdrive](https://github.com/dududaa/ppdrive) (the media option gates in `server/src/routers/upload.rs` are small and follow one pattern).

### 2. Project layout

Two layouts work; pick based on code sharing:

- **Single crate** — the plugin logic and the entry point live in one package (e.g. `audio-effects`). Simplest.
- **Library + thin cdylib** — when one library is linked into *several* plugins (e.g. `ppff-image-conversion` is linked into both image plugins), the `plugin_dispatch` symbol must **not** live in the shared rlib: two cdylibs in one process would define the same `#[no_mangle]` symbol and collide at link time. Put the symbol in a dedicated cdylib package (e.g. `image-conversion/plugin/`).

Cargo layout for the entry-point package:

```toml
[package]
name = "ppdrive-image-conversion"   # package name == full plugin id
version = "0.1.0"
edition = "2024"

[lib]
name = "image_conversion"           # short id, underscores instead of hyphens
crate-type = ["cdylib"]

[dependencies]
ppdrive = "0.2"                     # DispatchResponse + loader types (crates.io)
serde_json = "1"
# ... your engine (ffmpeg wrappers, image codecs, ...)
```

`cargo build --release --lib --package ppdrive-image-conversion` produces `libimage_conversion.so`; the CLI renames it to `ppdrive-image-conversion-linux-x86_64.so` when installing.

### 3. Implement `plugin_dispatch`

Signature and ownership rules:

```rust
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plugin_dispatch(args: *mut c_void) -> *mut DispatchResponse
```

- `args` is a `Box` of the argument tuple the server sends for your integration point — reclaim it with `Box::from_raw` **exactly once**.
- Return `Box::into_raw(Box::new(DispatchResponse::…))` — the server reclaims the returned pointer exactly once.
- `DispatchResponse::Ok(payload)` payload must be a `Box` of the type the server's dispatcher expects (`Box<Vec<u8>>` for both byte-oriented slots, `Box<Router<AppState>>` for the router slot).
- `DispatchResponse::Error(String)` surfaces your message to the server (logged; fails the inline operation).
- The library must stay valid for the whole call; the server keeps it loaded for its lifetime.

**Media shape** (upload processing and transformed downloads) — bytes in, bytes out:

```rust
use ppdrive::plugin::loader::DispatchResponse;
use serde_json::Value;
use std::ffi::c_void;

/// Args: (&[u8] input bytes, &Value option JSON).
/// Ok payload: Box<Vec<u8>> processed bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn plugin_dispatch(args: *mut c_void) -> *mut DispatchResponse {
    let args = unsafe { Box::from_raw(args as *mut (&[u8], &Value)) };
    let options = match serde_json::from_value::<MyOptions>(args.1.clone()) {
        Ok(options) => options,
        Err(err) => {
            return Box::into_raw(Box::new(DispatchResponse::Error(err.to_string())));
        }
    };
    let output: Vec<u8> = match transform(args.0, &options) {
        Ok(bytes) => bytes,
        Err(err) => return Box::into_raw(Box::new(DispatchResponse::Error(err.to_string()))),
    };
    let resp = DispatchResponse::Ok(Box::into_raw(Box::new(output)) as *mut c_void);
    Box::into_raw(Box::new(resp))
}
```

**File shape** (`media-streaming`) — paths in, package-description JSON out:

```rust
/// Args: (&Path input file, &Path output dir, &Value option JSON).
/// Ok payload: Box<Vec<u8>> of UTF-8 JSON {"playlist": "...", "files": [...]}.
let args = unsafe { Box::from_raw(args as *mut (&Path, &Path, &Value)) };
// ... write playlists/segments under args.1 ...
```

**Router shape** (`dashboard`) — state in, routes out (needs `ppdrive = { version = "0.2", features = ["server"] }` and the **same axum major version the server uses** — currently `0.8` — so the `Router<AppState>` type unifies across the dispatch):

```rust
use ppdrive::state::AppState;
use axum::Router;

/// Args: Box<AppState>. Ok payload: Box<Router<AppState>>.
let state = unsafe { Box::from_raw(args as *mut AppState) };
let router: Router<AppState> = build_router(*state);
let resp = DispatchResponse::Ok(Box::into_raw(Box::new(router)) as *mut c_void);
Box::into_raw(Box::new(resp))
```

### 4. Options contract

- The option JSON is the server's serialized session option — a fixed field set (all fields present, `null` for unset options) **plus** a `background` field the plugin should simply ignore.
- Deserialize with a plain `#[derive(Deserialize)]` struct of `Option<…>`/`Default` fields. **Do not use `deny_unknown_fields`** — the server may add timing or bookkeeping fields (`background`, and future ones) without breaking you.
- Session-time validation (content type family, ranges, operation arguments) happens **in the server before dispatch**; your plugin still must validate defensively — it can also be fed hand-written option JSON.
- Keep the options struct in sync with the [upload API docs](https://ppdrive.dududaa.org/api/upload/) for the slot you implement.

### 5. Ids, naming and release assets

| Thing | Convention | Example |
|---|---|---|
| Short id | hyphenated | `image-conversion` |
| Full id / cargo package | `ppdrive-<short id>` | `ppdrive-image-conversion` |
| `[lib]` name | short id, underscores | `image_conversion` |
| Installed library file | `ppdrive-<short id>-<os>-<arch>.<ext>` | `ppdrive-image-conversion-linux-x86_64.so` |

`<os>` is `linux` / `macos` / `windows`, `<arch>` is e.g. `x86_64` / `aarch64`, `<ext>` is `so` / `dylib` / `dll`. For remote installs the CLI downloads exactly this asset name from your repository's GitHub releases, so publish per-platform artifacts under those names (see ppff's release assets).

### 6. Build and install locally

```sh
# From a source directory (works for workspace subcrates — the CLI walks
# up to the workspace target/release to find the built library)
ppdrive plugin add <id> --source path/to/plugin/dir --local --build

# From a library you built yourself
ppdrive plugin add <id> --source ./ppdrive-<id>-linux-x86_64.so --local

ppdrive plugin list     # confirm the entry (ACTIVE: yes)
ppdrive serve           # plugins load at startup
```

Exercise the slot end to end: create an upload session with the matching option (or hit the download/dashboard route) and watch the server log for your dispatch errors.

### 7. Publish

1. Tag a release of your plugin repository.
2. Attach one asset per supported platform named `ppdrive-<id>-<os>-<arch>.<ext>` (CI example: ppff's `Release` workflow).
3. Users then install with `ppdrive plugin add <id> --source github:owner/repo` — no source toolchain required on the server.

Alternatively distribute source only and let users pass `--build` (remote source archives or `.git` URLs require it).

### Compatibility

- The dispatch ABI is plain C (`extern "C"`, `#[repr(C)]` response) and stable across PPDRIVE versions; the only symbol requirement is `plugin_dispatch`.
- Depend on the newest published `ppdrive` core crate for `DispatchResponse` (and `AppState` + the `server` feature for router plugins). Plugins built against the 0.2 core run unchanged on the 0.3 server — ppff's v0.3.0 plugins are built that way.
- Rebuild plugins when your platform, toolchain or engine (FFmpeg, codecs) changes; keep them on the same machine as the server, in `libs/`.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `<id> plugin is not installed` (400) | Not in the registry — `ppdrive plugin add <id> …` |
| `<id> plugin is installed but deactivated; run \`ppdrive plugin activate <id>\`` (400) | Deactivated — run the suggested command, restart |
| `<id> plugin failed to load` (400) | Registry entry exists but the library didn't load (wrong platform/arch, missing symbol, broken dependencies) — check the startup warnings in the server log, reinstall for this platform |
| `<option> requires content_type to be a video/* type, got '…'` (400) | Wrong MIME family for the option (`image/*`, `audio/*`, `video/*`, or video/audio for `media_streaming`) |
| `<option> only applies to file uploads` (400) | The option was sent on a `Folder` session |
| Plugin installed but seemingly ignored | Server not restarted — plugins load only at startup (`ppdrive serve`) |
| `plugin missing 'plugin_dispatch' function` in logs | Entry point missing, not `#[unsafe(no_mangle)]`, or the symbol is in a shared rlib linked into more than one cdylib |
| Inline upload fails with `500 upload failed` | Your plugin returned `DispatchResponse::Error` — the message is in the server log (`<id> failed: …`) |
| `ppdrive plugin update` can't fetch | Remote source recorded at install time is gone, or the platform has no matching release asset — reinstall, or add `--build` |

## See Also

- [Plugin CLI reference](https://ppdrive.dududaa.org/cli/plugin/) — full command/flag reference
- [Upload API](https://ppdrive.dududaa.org/api/upload/) — session options each media plugin receives
- [Download API](https://ppdrive.dududaa.org/api/download/) — transformed downloads
- [Configuration file](https://ppdrive.dududaa.org/configuration/config-file/) — `*_background` and cache knobs
- [ppff](https://github.com/dududaa/ppff) · [ppdrive-dashboard](https://github.com/dududaa/ppdrive-dashboard) — reference plugins
