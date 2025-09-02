# PPDRIVE (IN DEVELOPMENT)

**PPDRIVE** is a free and open-source cloud storage service written in Rust. PPDRIVE service provides a set of RESTful APIs that allows you to manage your remote and local files via secure and blazing fast operations.

If you're building your own version of Google Drive, Microsoft's OneDrive or even Dropbox, PPDRIVE provides a robust, secure, and extensible solution. You can also integrate PPDRIVE into your own applications as an alternative to AWS S3. Building self-hosted media streaming platforms? Build faster using PPDRIVE at zero cost.

---

## Why PPDRIVE?
- **Privacy and Data Control**: All your user, application and business data stay on your server. PPDRIVE does not host, spy or use your data in any way.
- **No vendor lock-in. No hidden charges**: Unlike AWS, GCP and other commercial alternatives, you don't need to pay for anything.
- **Full Ownership**: We don't collect any "insight" or gain access to any of your data.
- **Ease of Use**: Flexible service management tools for installing plugins, viewing or cancelling running services.
- **Security**: Secure by default.
- **Transparency**: The codebase is right here! Have a look at it, modify it and help us shape the future.
- **Speed**: Very fast operation handlers.
- **Lightweight**: We aim to make PPDRIVE as lightweight as possible. The starting point is to make sure you can install only what you need.

---

## 🚀 Features

- ⚙️ **REST API Interface**: Interact with the file system programmatically from any external application.
- 🔐 **Flexible Authentication**: PPDRIVE is secure by default, providing JWT authentication to protect private resources according to your applications. If you prefer to use PPDRIVE as a free tool and don't need protected access, you can opt-in for "No Auth" feature.
- 🪣 **Buckets**: Create isolated and configurable file buckets for better file system organization.
- 🧩 **Pluggable Integration**: Easily integrate into existing backends or operate as a standalone executable.
- 📦 **Prebuilt Executables**: Install and run with no setup required.
- 🧾 **File & Directory Operations**: Upload, download, rename, delete, and move files or folders.
- 🔒 **Secure by Default**: Runs with secure configurations and minimal surface area.
- 🛢 **Cross Database Support**: Support for Postgres, MySQL and SQLite (default) database backends.

#### Roadmap Features (Let's Build Together)
- Admin UI: Administrative client for managing your drive visually.
- File Compression: Compress files on the fly or at any convenient time.
- File Conversion: Convert files from one format to another.
- Image manipulation: Provide url queries for manipulating images.
- Async Upload: Upload large files in the background.
- File Streaming: Enable file streaming (for video/audio streaming platforms and other use cases)
- Live Streaming: End-to-end live streaming server.
- ~~Buckets: Buckets implementation for clients.~~
- ~~Mime Limits: Limit uploads to specified mime types.~~

---

## Installation
Documentation comming soon

---

## Support
If you would love to support by journey, please consider donating via any of these channels:

- **Bitcoin BNB Smart Chain (BEP20):** 0x912f022d95c31b983e5efe21bf22094ba6fafe55
- **Bitget BEP20 (Binance Smart Chain):** 0xd7a3da9faa4bd5c7ab0066391761a089e64a25d6

## Contributing
We welcome contributions to PPDRIVE! Here’s how you can help:

- You can create a PPDRIVE Client for your favorite language or framework. Please check our REST API Docs to get started.

- Or you can contribute directly to PPDRIVE by helping us resolve an issue or implement our roadmap feature. Please follow these steps.

1. Fork the repository.

2. Create a new branch (git checkout -b feature-name).

3. Make your changes.

4. Commit your changes (git commit -am 'Add new feature').

5. Push to your fork (git push origin feature-name).

6. Create a pull request.

---

## License
PPDRIVE is open-source software licensed under the [MIT License](#/LICENSE).

---

## Acknowledgments
- Dududaa: PPDRIVE is part of our ongoing Dududaa community project. You can [find us on Discord](https://discord.gg/BeVPFS7wkp).
- Rust: The systems programming language used to build PPDRIVE.
- Axum: A Rust-based web application framework that focuses on ergonomics and modularity.
- RBatis: RBatis serves as a drive for our modeller crate and record management within PPDRIVE.
- Docker: For containerizing the service.
- Community: Thanks to the open-source community for their valuable contributions!
