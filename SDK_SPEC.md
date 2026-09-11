# PPDRIVE Client SDK Specification

This document defines the architecture and API surface for PPDRIVE client SDKs. Use it as a reference when building an SDK in any programming language.

> **Language:** Code examples are shown in JavaScript/TypeScript for clarity. Adapt types, naming, and patterns to match your language's conventions.

---

## Table of Contents

- [Overview](#overview)
- [Client Construction](#client-construction)
- [API Methods](#api-methods)
  - [Upload](#upload)
  - [Download](#download)
  - [Buckets](#buckets)
  - [Permissions](#permissions)
- [Type Definitions](#type-definitions)
- [Error Handling](#error-handling)
- [Resumable Uploads](#resumable-uploads)
- [Range Downloads](#range-downloads)
- [Server Base URL](#server-base-url)
- [Naming Conventions](#naming-conventions)

---

## Overview

A PPDRIVE SDK wraps the HTTP API in a native-language client class. The SDK handles:

- HTTP requests and header management
- JSON serialization/deserialization
- Client token authentication
- Two-step upload and download flows (session creation + data transfer)
- Error mapping from HTTP status codes to typed exceptions

The SDK does **not** handle:

- Token generation or encryption (the server manages this)
- File I/O (the caller provides file data as bytes, streams, or paths)
- Server deployment or configuration

---

## Client Construction

### Constructor

```javascript
const client = new PPDRIVEClient({
  baseUrl: "http://localhost:8000",
  clientToken: "YOUR_CLIENT_TOKEN",
});
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `baseUrl` | `string` | Yes | PPDRIVE server URL (no trailing slash) |
| `clientToken` | `string` | Yes | Client API token for all operations |

### Static Factory (Optional)

If your language supports static methods, provide a convenience constructor:

```javascript
const client = PPDRIVEClient.fromToken("http://localhost:8000", "CLIENT_TOKEN");
```

---

## API Methods

### Upload

#### `uploadFile(path, data, options?)`

Upload a file to the server. This is a two-step operation: create session, then send data.

```javascript
// Simple upload
await client.uploadFile("docs/report.pdf", fileBytes);

// Upload to a bucket
await client.uploadFile("report.pdf", fileBytes, {
  bucket: "BUCKET_PID",
  contentType: "application/pdf",
  overwrite: true,
});

// Upload with progress
await client.uploadFile("large.bin", fileBytes, {
  bucket: "BUCKET_PID",
  resumable: true,
  onProgress: (sent, total) => console.log(`${sent}/${total}`),
});
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | `string` | Yes | Destination path (relative to bucket or static dir) |
| `data` | `bytes` | Yes | File content as bytes, stream, or buffer |
| `options.bucket` | `string` | No | Bucket PID (omit for static directory upload) |
| `options.contentType` | `string` | No | MIME type (required if bucket has `accepts` restrictions) |
| `options.overwrite` | `boolean` | No | Allow overwriting existing file (default: `false`) |
| `options.createParents` | `boolean` | No | Create parent directories (default: `false`) |
| `options.resumable` | `boolean` | No | Enable chunked upload (default: `false`, required for files ≥ 2MB) |
| `options.chunkSize` | `number` | No | Bytes per chunk for resumable uploads (default: 1MB) |
| `options.expires` | `number` | No | Session lifetime in seconds (default: `120`, range: `30–86400`) |
| `options.onProgress` | `function` | No | Progress callback `(bytesSent, totalBytes) → void` |

**Returns:** `void`

**HTTP:** `POST /upload/session` → `POST /upload/session/play/{token}`

---

#### `uploadFolder(path, options?)`

Create a folder on the server.

```javascript
await client.uploadFolder("documents/2024/reports");
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | `string` | Yes | Folder path |
| `options.bucket` | `string` | No | Bucket PID |
| `options.overwrite` | `boolean` | No | Allow overwriting (default: `false`) |
| `options.createParents` | `boolean` | No | Create parent directories (default: `false`) |

**Returns:** `void`

**HTTP:** `POST /upload/session` (asset_type: "Folder") → `POST /upload/session/play/{token}`

---

### Download

#### `downloadFile(bucketPid, path, options?)`

Download a file from a private bucket. Returns the file bytes.

```javascript
// Full download
const data = await client.downloadFile("BUCKET_PID", "report.pdf");
require("fs").writeFileSync("report.pdf", data);

// Download with range
const partial = await client.downloadFile("BUCKET_PID", "video.mp4", {
  range: { start: 0, end: 1023 },
});

// Download as stream (if your language supports it)
const stream = await client.downloadFileStream("BUCKET_PID", "video.mp4");
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bucketPid` | `string` | Yes | Bucket PID |
| `path` | `string` | Yes | File path within the bucket |
| `options.range` | `object` | No | `{ start: number, end?: number }` for partial download |
| `options.expires` | `number` | No | Token lifetime in seconds (default: `300`, range: `30–3600`) |

**Returns:** `bytes` (or `Stream` if streaming is preferred)

**HTTP:** `POST /download/sign` → `GET /download/{token}`

---

#### `downloadPublicFile(url)`

Download a file from a public bucket or static directory. No authentication needed.

```javascript
const data = await client.downloadPublicFile("http://localhost:8000/storage/images/photo.jpg");
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `url` | `string` | Yes | Full URL to the public file |

**Returns:** `bytes`

**HTTP:** `GET {url}`

---

### Buckets

#### `createBucket(options)`

Create a new storage bucket.

```javascript
const bucketPid = await client.createBucket({
  name: "Documents",
  path: "storage/documents",
  public: false,
  size: 1024,  // MB
  accepts: ["application/pdf", "image/*"],
});
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `options.name` | `string` | Yes | Bucket display name (1–255 chars) |
| `options.path` | `string` | Yes | Filesystem path for stored files |
| `options.public` | `boolean` | No | Make bucket publicly accessible (default: `false`) |
| `options.size` | `number` | No | Max bucket size in MB (omit for unlimited) |
| `options.accepts` | `string[]` | No | Accepted MIME types (exact or `type/*` wildcards) |

**Returns:** `string` (bucket PID)

**HTTP:** `POST /buckets`

---

### Permissions

#### `grantPermission(bucketPid, options)`

Grant a file-level permission to a client or user.

```javascript
// Grant read access to a client
await client.grantPermission("BUCKET_PID", {
  path: "report.pdf",
  grantee: "CLIENT_PID",
  granteeType: "client",
  permission: "read",
});

// Grant admin access to a user
await client.grantPermission("BUCKET_PID", {
  path: "report.pdf",
  grantee: "user@example.com",
  granteeType: "user",
  permission: "admin",
});
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bucketPid` | `string` | Yes | Bucket PID |
| `options.path` | `string` | Yes | File path within the bucket |
| `options.grantee` | `string` | Yes | Client PID or user email |
| `options.granteeType` | `string` | Yes | `"client"` or `"user"` |
| `options.permission` | `string` | Yes | `"read"`, `"write"`, or `"admin"` |

**Returns:** `void`

**HTTP:** `POST /buckets/{bucket_pid}/permissions`

---

#### `revokePermission(bucketPid, options)`

Revoke a previously granted permission.

```javascript
await client.revokePermission("BUCKET_PID", {
  path: "report.pdf",
  grantee: "user@example.com",
  granteeType: "user",
});
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bucketPid` | `string` | Yes | Bucket PID |
| `options.path` | `string` | Yes | File path within the bucket |
| `options.grantee` | `string` | Yes | Client PID or user email |
| `options.granteeType` | `string` | Yes | `"client"` or `"user"` |

**Returns:** `void`

**HTTP:** `DELETE /buckets/{bucket_pid}/permissions`

---

#### `listPermissions(bucketPid, path?)`

List all permissions for a file.

```javascript
const permissions = await client.listPermissions("BUCKET_PID", "report.pdf");
// [
//   { id: 1, grantee: "CLIENT_PID", granteeType: "client", permission: "admin", ... },
//   { id: 2, grantee: "user@example.com", granteeType: "user", permission: "read", ... },
// ]
```

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `bucketPid` | `string` | Yes | Bucket PID |
| `path` | `string` | No | Filter by specific file path |

**Returns:** `Permission[]`

**HTTP:** `GET /buckets/{bucket_pid}/permissions?path=...`

---

## Type Definitions

Adapt these to your language's type system (interfaces, structs, classes, etc.).

```typescript
interface PPDRIVEClientOptions {
  baseUrl: string;
  clientToken: string;
}

interface UploadOptions {
  bucket?: string;
  contentType?: string;
  overwrite?: boolean;
  createParents?: boolean;
  resumable?: boolean;
  chunkSize?: number;     // bytes, default 1MB
  expires?: number;       // seconds, default 120
  onProgress?: (bytesSent: number, totalBytes: number) => void;
}

interface DownloadOptions {
  range?: { start: number; end?: number };
  expires?: number;  // seconds, default 300
}

interface CreateBucketOptions {
  name: string;
  path: string;
  public?: boolean;
  size?: number;     // MB
  accepts?: string[];
}

interface GrantPermissionOptions {
  path: string;
  grantee: string;
  granteeType: "client" | "user";
  permission: "read" | "write" | "admin";
}

interface RevokePermissionOptions {
  path: string;
  grantee: string;
  granteeType: "client" | "user";
}

interface Permission {
  id: number;
  assetId: number;
  granteeId: number;
  granteeType: "client" | "user";
  granteeName: string;
  permission: "read" | "write" | "admin";
  createdAt: string;
}
```

---

## Error Handling

The SDK should throw a typed error for all API failures.

### Error Class

```javascript
class PPDRIVEError extends Error {
  constructor(status, message) {
    super(message);
    this.name = "PPDRIVEError";
    this.status = status;   // HTTP status code
    this.message = message; // Server error message
  }
}
```

### Status Code Mapping

| Status | Error Type | Description |
|--------|-----------|-------------|
| `400` | `ValidationError` | Bad request parameters |
| `401` | `AuthenticationError` | Invalid or missing credentials |
| `403` | `AuthorizationError` | Insufficient permissions |
| `404` | `NotFoundError` | Resource not found |
| `409` | `ConflictError` | Resource already exists |
| `413` | `PayloadTooLargeError` | File too large |
| `416` | `RangeError` | Invalid range header |
| `429` | `RateLimitError` | Too many requests |
| `500` | `ServerError` | Internal server error |

### Example Error Handling

```javascript
try {
  await client.uploadFile("report.pdf", data, { bucket: "PID" });
} catch (err) {
  if (err.status === 403) {
    console.log("Access denied:", err.message);
  } else if (err.status === 413) {
    console.log("File too large, use resumable upload");
  } else {
    throw err;
  }
}
```

---

## Resumable Uploads

For files ≥ 2 MB, enable `resumable: true`. The SDK handles chunking internally.

### Flow

```
1. Create session (resumable: true)
2. Send chunk 0 → receive token 1
3. Send chunk 1 → receive token 2
4. ...repeat until complete
5. Final chunk returns null (upload done)
```

### SDK Behavior

- Split file into chunks of `chunkSize` bytes (default: 1 MB)
- Send each chunk to `POST /upload/session/play/{token}`
- Use the returned token for the next chunk
- Call `onProgress` after each chunk
- If the process is interrupted, the caller can resume by storing the last token and calling `resumeUpload(lastToken, remainingData)`

### Optional: Resume Support

```javascript
// Store this between sessions
const lastToken = await client.uploadFile("large.bin", chunk1, {
  resumable: true,
  onProgress: (sent, total, nextToken) => {
    // Save nextToken to database/disk for resume
    saveResumeToken(nextToken);
  },
});

// Resume later
const resumeToken = loadResumeToken();
await client.resumeUpload(resumeToken, remainingChunks);
```

---

## Range Downloads

Download partial file content using HTTP Range headers.

### Examples

```javascript
// First 1024 bytes
const head = await client.downloadFile("PID", "file.bin", {
  range: { start: 0, end: 1023 },
});

// Last 1024 bytes
const tail = await client.downloadFile("PID", "file.bin", {
  range: { start: -1024 },
});

// Bytes 1024-2047
const middle = await client.downloadFile("PID", "file.bin", {
  range: { start: 1024, end: 2047 },
});
```

---

## Server Base URL

The SDK should accept the base URL without a trailing slash:

```
http://localhost:8000       ✓
http://localhost:8000/      ✗ (strip trailing slash)
```

Append paths as defined in the API method sections above.

---

## Naming Conventions

Adapt to your language's conventions:

| Concept | JavaScript/TypeScript | Python | Go | Rust |
|---------|----------------------|--------|----|------|
| Class | `PPDRIVEClient` | `PPDRIVEClient` | `Client` | `Client` |
| Constructor | `new PPDRIVEClient(opts)` | `PPDRIVEClient(opts)` | `NewClient(opts)` | `Client::new(opts)` |
| Methods | `camelCase` | `snake_case` | `PascalCase` | `snake_case` |
| Error | `PPDRIVEError` | `PPDRIVEError` | `*Error` types | `Error` enum |
| Options | `UploadOptions` | `UploadOptions` | `UploadOpts` | `UploadOptions` |

### Package Naming

| Language | Package Name |
|----------|-------------|
| JavaScript/TypeScript | `ppdrive-sdk` |
| Python | `ppdrive-sdk` or `ppdrive` |
| Go | `github.com/ppdrive/ppdrive-go-sdk` |
| Rust | `ppdrive-sdk` |
| Java/Kotlin | `ppdrive-sdk` |
| Dart | `ppdrive_sdk` |
| C# | `PPDRIVE.Sdk` |
| Ruby | `ppdrive-sdk` |
| PHP | `ppdrive/sdk` |

---

## Testing

Each SDK should include tests for:

1. **Unit tests** — Mock HTTP responses, verify request formatting
2. **Integration tests** — Run against a local PPDRIVE server (use `ppdrive serve` in test setup)
3. **Upload tests** — Small file, large resumable file, folder creation
4. **Download tests** — Public file, private file, range download
5. **Error tests** — Invalid token, expired token, permission denied, not found

### Test Server Setup

```bash
# Start a test server
ppdrive serve --port 8000 &

# Create a test client
ppdrive client create --name "SDK Tests"

# Run tests with the client token
TEST_TOKEN="..." npm test
```

---

## Checklist for SDK Authors

- [ ] Create repo named `ppdrive-[language]-sdk`
- [ ] Implement all methods from this spec
- [ ] Handle all error status codes
- [ ] Support resumable uploads (chunked)
- [ ] Support range downloads
- [ ] Include README with installation and usage examples
- [ ] Add tests (unit + integration)
- [ ] Publish to language's package registry
- [ ] Open PR on [ppdrive/ppdrive](https://github.com/dududaa/ppdrive) to add to the SDK list
