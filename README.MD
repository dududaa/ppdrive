# PPDRIVE (IN DEVELOPMENT)

**PPDRIVE** is a free and open-source cloud storage service written in Rust. PPDRIVE service provides a set of RESTful APIs that allows you to manage your remote and local files via secure and blazing fast operations.

If you're building your own version of Google Drive, Microsoft's OneDrive or even Dropbox, PPDRIVE provides a robust, secure, and extensible solution. You can also integrate PPDRIVE into your own applications as an alternative to AWS S3. Building self-hosted media streaming platforms? Build faster using PPDRIVE at zero cost.

---

## Why PPDRIVE?
- **Privacy and Data Control**: All your data on your server.
- **Full Ownership**: We don't have access to any of your data.
- **Ease of User**: Flexible service management tools.
- **Security**: Secure by default.
- **Transparency**: The codebase is right here! Have a look at it, modify it and help us shape the future.
- **Speed**: Fast operation handlers.
- **Lightweight**: We aim to make PPDRIVE lightweight. The starting point is to make sure you can install only what you need.

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
- ~~Buckets: Buckets implementation for clients.~~
- Mime Limits: Limit uploads to specified mime types.

---

## Installation
#### Linux and MacOS
**Prequisite**: Please make sure you have created a database for PPDRIVE to save records. You'll need to add the database URL after installation, during configuration. We support Postgres, MySQL and SQLite databases.

Once your database is ready, kindly proceed to the following steps:

- Install:
```bash
curl -f https://raw.githubusercontent.com/prodbyola/ppdrive/refs/heads/main/install.sh | sh
```

- Configure:
```bash
ppdrive configure
```

- Start
```bash
systemctl --user start ppdrive
```

#### Docker Compose
If you have docker compose, you can use our [compose template](https://raw.githubusercontent.com/prodbyola/ppdrive/refs/heads/main/compose.yml) for full installation (database included). Copy and modify the [compose template](https://raw.githubusercontent.com/prodbyola/ppdrive/refs/heads/main/compose.yml) to suit your need. Take note to update `ppdrive_db` service environments and `PPDRIVE_DATABASE_URL` in `ppdrive` service.

- Build: Build docker containers
```sh
docker-compose up --build -d
```

- Configure: Open `ppdrive` container terminal and run the configuration command.
```sh
docker-compose exec ppdrive sh

ppdrive --configure
```

- Restart: Now restart the containers
```sh
docker-compose restart
```

---

## API Documentation
Once your PPDRIVE is pu and running, check out [API DOC](https://prodbyola.github.io/ppdrive/) to learn how to connect and use your PPDRIVE instance.

---

---

## Running Tests

1. First, run migration test: `cargo test test_migration`.
2. Then run other tests, excluding migration test `cargo test -- --skip test_migration`
---

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
