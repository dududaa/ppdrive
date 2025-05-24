# PPDRIVE

**PPDRIVE** is a powerful, standalone file manager written in Rust, designed for seamless integration into other applications via a RESTful API. Whether you're building a complex backend system or need a standalone file server with authentication and file-level permissions, PPDRIVE provides a robust, secure, and extensible solution.

---

## 🚀 Features

- ⚙️ **REST API Interface**: Interact with the file system programmatically from any external application.
- 🔐 **Authentication & Authorization**: Built-in support for user authentication and per-file permissions. PPDRIVE supports peer-to-peer file sharing through asset-level permissions.
- 🧩 **Pluggable Integration**: Easily integrate into existing backends or operate as a standalone executable.
- 📦 **Prebuilt Executables**: Install and run with no setup required.
- 🧾 **File & Directory Operations**: Upload, download, rename, delete, and move files or folders.
- 🔒 **Secure by Default**: Runs with secure configurations and minimal surface area.
- 🛢 **Cross Database Support**: Support for Postgres, MySQL and SQLite database backends. Thanks to Sqlx's [Any](https://docs.rs/sqlx/latest/sqlx/any/index.html) feature.

#### Features (Roadmap)
- Admin UI: Administrative client for managing your drive visually.
- File Compression: Compress files on the fly or at any convenient time.
- File Conversion: Convert files from one format to another.
- Async Upload: Upload large files in the background.
- File Streaming: Enable file streaming (for video and audio streaming platforms)

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

## Contributing
We welcome contributions to PPDRIVE! Here’s how you can help:

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
- Basable: PPDRIVE is part of our ongoing Basable community project. You can [find us on Discord](https://discord.gg/BeVPFS7wkp).
- Rust: The systems programming language used to build PPDRIVE.
- Axum: A Rust-based web application framework that focuses on ergonomics and modularity.
- Sqlx: The async SQL toolkit for Rust, built with ❤️ by the LaunchBadge team.
- Docker: For containerizing the service.
- Community: Thanks to the open-source community for their valuable contributions!
