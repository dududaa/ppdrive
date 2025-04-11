# PPDRIVE

**PPDRIVE** is a self-hosted, self-contained and pluggable service for digital assets management. It allows you to seamlessly upload, manage, store, and access your application files, attachments and other digital assets providing flexibile integration into various environments.

---

## Features

- **Self-Hosted**: Set up and run your own instance of PPDRIVE on your server.
- **Self-Contained**: Everything you need is packaged together into a single executable program, reducing external dependencies.
- **Pluggable**: Easily integrate PPDRIVE into your existing applications and favorite frameworks.
- **Secure**: Provides robust security features (JWT authentication support, user permissions...etc) to keep your assets safe.
- **Scalable**: Designed for scalability, making it suitable for both small and large projects.
- **REST API**: Access your assets programmatically through a simple and well-documented REST API.

---

## Getting Started

These instructions will help you set up and run PPDRIVE on your server.

### Prerequisites

Before you begin, ensure you have the following installed:

- **Rust** (Version 1.XX or later): You can install Rust via [rustup](https://rustup.rs/).
- **Docker** (optional, for containerized deployment): Install Docker from [docker.com](https://www.docker.com/).

### Installation

#### Clone the Repository

```bash
git clone https://github.com/your-username/ppdrive.git
cd ppdrive
```

#### Build and Run the Service
To build and run PPDRIVE:
```bash
cargo build --release
cargo run
```

#### Running with Docker (Optional)
To run PPDRIVE in a Docker container, use the following commands:
```bash
docker compose up
```

## Configuration
PPDRIVE comes with a default configuration file. You can modify the configuration by editing the config.toml file.

Example:
```toml
[server]
host = "0.0.0.0"
port = 8080
```

## API Documentation
PPDRIVE exposes a simple and flexible REST API to manage your digital assets.

#### Authentication
All API requests require authentication. You can configure the authentication mechanism in config.toml.

#### Endpoints
Here are some key API endpoints:

- POST /assets/upload: Upload a new asset.

- GET /assets/{id}: Retrieve asset metadata.

- GET /assets/{id}/download: Download the asset.

- DELETE /assets/{id}: Delete an asset.

For a full list of available API endpoints and their usage, refer to the API Documentation.

## Contributing
We welcome contributions to PPDRIVE! Here’s how you can help:

1. Fork the repository.

2. Create a new branch (git checkout -b feature-name).

3. Make your changes.

4. Commit your changes (git commit -am 'Add new feature').

5. Push to your fork (git push origin feature-name).

6. Create a pull request.

## License
PPDRIVE is open-source software licensed under the MIT License.

## Acknowledgments
- Rust: The systems programming language used to build PPDRIVE.
- Docker: For containerizing the service.
- Community: Thanks to the open-source community for their valuable contributions!