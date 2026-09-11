[![Actions Status](https://github.com/dududaa/ppdrive/workflows/Release%20PPDRIVE/badge.svg)](https://github.com/dududaa/ppdrive/actions) [![Actions Status](https://github.com/dududaa/ppdrive/workflows/Release%20Dynamic%20Libraries/badge.svg)](https://github.com/dududaa/ppdrive/actions)

# PPDRIVE

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
- [ ] Client SDKs (Node.js, Python, Go, Kotlin, Rust, or any language — see [SDK_SPEC.md](SDK_SPEC.md))
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

## Client SDKs

PPDRIVE works with any language that can send HTTP requests. We encourage developers to build client SDKs for their language of choice.

### Available SDKs

| Language | Repository | Author |
|----------|-----------|--------|
| *(none yet — be the first!)* | | |

### Build an SDK?

1. Read the [SDK Specification](SDK_SPEC.md) for the full API architecture
2. Create a repo named `ppdrive-[language]-sdk`
3. Implement the SDK following the spec
4. Open a PR here to add your SDK to the list above

See the [SDK Contributor Guide](https://ppdrive.dududaa.org/contributing/sdk/) for detailed instructions.

## Contributing

Please see [Contribution Guide](https://ppdrive.dududaa.org/contributing/development/) for detailed contribution guide.

---

## License

PPDRIVE is open-source software licensed under the [PolyForm Noncommercial License](https://polyformproject.org/licenses/noncommercial/1.0.0). Commercial use requires permission — see [LICENSE](LICENSE) for details.

---

## Acknowledgments

We ride on the shoulders of giants! We'll just mention a few:

- Dududaa: PPDRIVE is part of our ongoing Dududaa community project. You
  can [find us on Discord](https://discord.gg/6nB4xYnxeC).
- Rust: The systems programming language used to build PPDRIVE.
- Tokio: For Axum web framework and several other crates that simplify asynchronous Rust.
- Community: Thanks to the open-source community for their valuable contributions!
