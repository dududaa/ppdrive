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

###### Installation
The installation script has only been tested on Linux but should work on other platforms. If you experienced any issue, please
download platform's binary on [release page](https://github.com/dududaa/ppdrive/releases) or simply build from source.

**Linux / macOS**
```sh
# Using curl (Linux)
curl -fsSL https://raw.githubusercontent.com/dududaa/ppdrive/main/install.sh | bash

# Using wget (macOS/Linux)
wget -qO- https://raw.githubusercontent.com/dududaa/ppdrive/main/install.sh | bash

# Using Homebrew (macOS)
brew tap dududaa/ppdrive && brew install ppdrive
```

**Windows**

```shell
powershell -NoProfile -ExecutionPolicy Bypass -Command "iwr -useb https://raw.githubusercontent.com/dududaa/ppdrive/main/install.ps1 | iex"
```

Type `ppdrive --help` to check if all goes well.

---

#### 🚀 Getting Started

PPDRIVE takes less than 2 minutes to set up. Create your own Object Storage service and buckets in seconds - no vendor lock-in, no hidden costs.

###### Basic Usage
###### Step 1: Create Your First Client
Create authentication tokens for your applications:
```shell
# Create a new client with name "myapp"
ppdrive client create --name myapp --max-bucket-size 100

# Output:
# Client created successfully!
# Client ID: gctR57Tekg
# Client Token: 3add30b582768e...
```

To regenerate token for an existing client (in case of compromise):
```shell
# Create a new client with name "myapp"
ppdrive client refresh --id gctR57Tekg

# Output:
# Client token refreshed successfully!
# Client Token: 4adb30b582768e...
```

###### Step 2: Create Your First Client
Now that your client token is ready, start the server:
```shell
# Start the API server
ppdrive serve
```

The command above will attempt to start the server on port `8000`. You can specify service port:
```shell
# Start the API server
ppdrive serve --port 3000
```

#### Roadmap Features (Let's Build Together)

- [ ] Private object access.
- [ ] Image transformation.
- [ ] Image conversion.
- [ ] File conversion.
- [ ] File compression.
- [ ] Resumable Upload.
- [ ] Live File Streaming.
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
  can [find us on Discord](https://discord.gg/BeVPFS7wkp).
- Rust: The systems programming language used to build PPDRIVE.
- Tokio: For Axum web framework and several other crates that simplify asynchronous Rust.
- RBatis: RBatis serves as a drive for our modeller crate and record management within PPDRIVE.
- Community: Thanks to the open-source community for their valuable contributions!
