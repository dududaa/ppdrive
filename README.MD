# PPDRIVE (IN DEVELOPMENT)

**PPDRIVE** is a free, open-source cloud storage service built with Rust for speed, security, and reliability. While you can use it as a standalone service, think of PPDRIVE as a framework but for filesystem management that your applications can use to manage remote and local files using secure and lightening-fast operations.

Whether you're building your own version of Google Drive, OneDrive, or Dropbox, PPDRIVE gives you robust, secure, and extensible APIs that allow users to interact seamlessly with your server’s file system at the application level.

Need a self-hosted alternative to AWS S3? PPDRIVE integrates effortlessly into your existing applications. From self-hosted media streaming platforms to enterprise-grade storage solutions, PPDRIVE helps you build faster, stay in control, and pay nothing.

---

## Motivation
We notice that many startups, organizations, and developers prefer not to rely on third-party cloud storage services due to concerns about long-term costs, data privacy, and vendor lock-in. Unfortunately, the lack of viable alternatives often leaves them with limited choices. PPDRIVE was created to address this gap — offering a flexible, self-hosted suite of APIs for remote file management and storage.

---

## Why PPDRIVE?
- **Language/Framework Agnostic**: PPDRIVE works with any programming language or framework that allows you to send http requests. Whether it's Javascript or Golang; React, Vue, Angular, Flutter...you name it. You can even help us write an SDK or client library for your favorite language.
- **Privacy and Data Control**: All your user, application and business data stay on your server. PPDRIVE does not send, spy or use your data in any way.
- **No vendor lock-in. No hidden charges**: You don't need to pay for anything. PPDRIVE is totally free.
- **User-level Authorization**: PPDRIVE automatically provides fine-grained resource ownership so that only authorized users can access to assets. This way, you can focus on building other aspects of your application as we help you protect your filesystem.
- **Ease of Use**: If you're already familiar with sending HTTP requests, using PPDRIVE feels natural. We also provide a flexible service management tool for starting multiple services, viewing or cancelling running services.
- **Transparency**: The codebase is right here! Have a look at it, modify it and help us shape the future.
- **Speed**: Very fast operation handlers.
- **Lightweight**: We aim to make PPDRIVE as lightweight as possible. The starting point is to make sure you can install only what you need and extend it whenever you need more.

---

## 🚀 Features

- ⚙️ **REST API Interface**: PPDRIVE allows external applications to interact with your file system programmatically RESTful APIs. However, it is designed to be extended to support other networking protocols.
- 🔐 **Flexible Authentication**: PPDRIVE is secure by default, providing JWT authentication to protect private resources according to your applications. If you prefer to use PPDRIVE as a free tool and don't need protected access, you can opt-in for "No Auth" feature.
- 🪣 **Buckets**: Create isolated and configurable file buckets for better file system organization.
- 🧩 **Pluggable Integration**: Easily integrate into existing backends or operate as a standalone executable.
- 📦 **Prebuilt Executables**: Install and run with no setup required.
- 🧾 **File & Directory Operations**: Upload, download, rename, delete, and move files or folders.
- 🛢 **Cross Database Engine Support**: Support for Postgres, MySQL and SQLite (default) database backends.

#### Roadmap Features (Let's Build Together)
- Admin UI: Administrative client for managing your drive visually.
- File Compression: Compress files on the fly or at any convenient time.
- File Conversion: Convert files from one format to another.
- Image manipulation: Provide url queries for manipulating images.
- Async Upload: Upload large files in the background.
- File Streaming: Enable file streaming (for video/audio streaming platforms and other use cases)
- Live Streaming: End-to-end live streaming server.

---

## Getting Started
#### 1. Install
Run the following commands to install PPDRIVE.
###### Linux
curl:
```sh
curl -fsSL https://raw.githubusercontent.com/dududaa/ppdrive/main/install.sh | bash
```

wget:
```sh
wget -qO- https://raw.githubusercontent.com/dududaa/ppdrive/main/install.sh | bash
```

###### MacOS
homebrew:
```sh
brew tap dududaa/ppdrive && brew install ppdrive
```

###### Windows
powershell:
```shell
powershell -NoProfile -ExecutionPolicy Bypass -Command "iwr -useb https://raw.githubusercontent.com/dududaa/ppdrive/main/install.ps1 | iex"
```
#### 2. Start
```sh
ppdrive start
```

#### 3. Launch
```sh
ppdrive launch rest
```

Please see [Getting Started](/docs/GETTING-STARTED.MD) for complete starter guide.

---

## Support
If you would love to support by journey, please consider donating via any of these channels:

- **Bitcoin BNB Smart Chain (BEP20):** 0x912f022d95c31b983e5efe21bf22094ba6fafe55
- **Bitget BEP20 (Binance Smart Chain):** 0xd7a3da9faa4bd5c7ab0066391761a089e64a25d6

---

## Contributing
We welcome contributions to PPDRIVE! Here’s how you can help:

- You can create a PPDRIVE Client for your favorite language or framework. Please check our REST API Docs to get started.

- You can help improve code-level documentations.

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
We ride on the shoulders of giants! We'll just mention a few:
- Dududaa: PPDRIVE is part of our ongoing Dududaa community project. You can [find us on Discord](https://discord.gg/BeVPFS7wkp).
- Rust: The systems programming language used to build PPDRIVE.
- Tokio: For Axum web framework and several other crates that simplify asynchronous Rust.
- RBatis: RBatis serves as a drive for our modeller crate and record management within PPDRIVE.
- Community: Thanks to the open-source community for their valuable contributions!
