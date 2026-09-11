[![Actions Status](https://github.com/dududaa/ppdrive/workflows/Release%20PPDRIVE/badge.svg)](https://github.com/dududaa/ppdrive/actions) [![Actions Status](https://github.com/dududaa/ppdrive/workflows/Release%20Dynamic%20Libraries/badge.svg)](https://github.com/dududaa/ppdrive/actions)

# PPDRIVE (Work In Progress)

**PPDRIVE** is an open-source Object Storage Service designed to let you deploy a fully functional remote file storage
system in seconds. It provides a ready-made solution for uploading, downloading, and managing files, enabling your
applications to connect and start using storage immediately.

PPDRIVE hopes to offer functionality comparable to commercial services like Amazon S3 buckets, making it a lightweight and flexible alternative for developers who want full control over their storage infrastructure.

---

## Motivation

We notice that many startups, organizations, and developers prefer not to rely on third-party cloud storage services due
to concerns about long-term costs, data privacy, and vendor lock-in. Unfortunately, the lack of viable alternatives
often leaves them with limited choices. PPDRIVE was created to address this gap — offering a flexible, self-hosted suite
of APIs for remote file management and storage.

---

## Why PPDRIVE?

- **Language/Framework Agnostic**: PPDRIVE works with any programming language or framework that allows you to send http
  requests. Whether it's Javascript or Golang; React, Vue, Angular, Flutter...you name it. You can even help us write an
  SDK or client library for your favorite language.
- **Privacy and Data Control**: All your user, application and business data stay on your server. PPDRIVE does not send,
  collect or use your data in any way.
- **No vendor lock-in. No hidden charges**: You don't need to pay for anything. PPDRIVE is totally free.
- **User-level Authorization**: PPDRIVE automatically provides fine-grained resource ownership so that only authorized
  users can access to assets. 
- **Ease of Use**: If you're already familiar with sending HTTP requests, using PPDRIVE feels natural. We also provide a
  cli-tool for configuration, client management, plugin management and so on.
- **Transparency**: The codebase is right here! Have a look at it, modify it and help us shape the future.
- **Speed**: Very fast operation handlers.
- **Cross Platform**: PPDRIVE can run on Linux, MacOS and Windows. Useful for local development.
- **Lightweight**: We aim to make PPDRIVE as lightweight as possible. The starting point is to make sure you can install
  only what you need and extend it whenever you need more.

---

###### Documentation
Please see the [Documentation Website](https:/ppdrive.dududaa.org).

#### Roadmap Features (Let's Build Together)

- [x] Private object access.
- [x] Resumable Upload.
- Client SDKs
  - [ ] Node.js
  - [ ] Python
  - [ ] GoLang
  - [ ] Kotlin
  - [ ] Rust
- Extensions
  - [ ] Dashboard UI
  - [ ] Image transformation.
  - [ ] Image conversion.
  - [ ] Video Transformation.
  - [ ] File conversion.
  - [ ] File compression.
  - [ ] Live File Streaming.
  - [ ] PDF Utilities
  - [ ] And many more.

---

## Contributing

We welcome contributions to PPDRIVE! Here’s how you can help:

- You can create a PPDRIVE Client for your favorite language or framework. Please check our REST API Docs to get
  started.

- You can help improve code-level documentations.

- Or you can contribute directly to PPDRIVE by helping us resolve an issue or implement our roadmap feature. Please
  follow these steps.

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

- Dududaa: PPDRIVE is part of our ongoing Dududaa community project. You
  can [find us on Discord](https://discord.gg/6nB4xYnxeC).
- Rust: The systems programming language used to build PPDRIVE.
- Tokio: For Axum web framework and several other crates that simplify asynchronous Rust.
- Community: Thanks to the open-source community for their valuable contributions!
