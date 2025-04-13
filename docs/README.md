# Getting Started

These instructions will help you set up and run PPDrive on your server.

Before you begin, please note that you can install PPDRIVE either with or without Docker Compose. For the easiest setup, we recommend using Docker Compose. However, we’ve provided step-by-step guides for both installation methods to suit your preference.

### Installation with Docker (Recommended)
#### 1. Install Docker and Docker Compose
Checkout installation guides for [Docker](https://docs.docker.com/engine/install/) and [Docker Compose](https://docs.docker.com/compose/install/). If you're on your local machine, you can [download and install Docker Engine](https://www.docker.com/get-started/) instead, which comes with both Docker and Compose.

#### 2. Clone the Repository
Once Docker Compose is installed, let's download PPDRIVE's repository. 

```bash
git clone https://github.com/prodbyola/ppdrive.git
cd ppdrive
```

#### 3. Copy Example Configuration
PPDRIVE configurations are saved in a `.env` file. We provide an example configuration that you can copy and customize to suit your project’s needs.

```bash
cp .example.env .env
```

#### 4. Update Configuration
Now update the configuration file. Please see [Configuration](/configuration) for more.
```bash
nano .env
```

#### 5. Build and Deploy
Once you're satisfied with your configurations, build and deploy ppdrive.
```bash
docker compose up --build -d
```

#### 6. Verify Installation Success
To verify that installation is successful, check docker container's log.
```bash
docker compose logs ppdrive
```
On successful installation, you should see a message like this:
```bash
ppdrive_app  | 2025-04-12T09:50:22.229817Z  INFO ppdrive: listening on 0.0.0.0:5000
```
Congratulations! The message indicates a PPDRIVE instance is running on port `5000` (or any other port you specified in [Configuration](/configuration)).


### Running Without Docker
If you want to install PPDRIVE without Docker, please follow this guide carefully.

#### 1. Setup Database
PPDRIVE uses Postgresql to manage application records internally (users, clients, assets, permissions...etc). FOllow these steps to setup your database.
- If you haven't already, [download and install Postgresql](https://www.postgresql.org/download/)
- Once installed, login as root user `psql -U postgres`
- Create a new user `CREATE USER your_username WITH PASSWORD 'your_password';`. Write down `your_username` and `your_password` as you'll be needing them later.
- Now create a new database for PPDRIVE `CREATE DATABASE your_database;`. Write down `your_database` as well.
- Grant full priviledge on database to the user `GRANT ALL PRIVILEGES ON DATABASE your_database TO your_username;`

#### 2. Download PPDRIVE
Visit our [releases page](https://github.com/prodbyola/ppdrive/releases) and download PPDRIVE version of your choice. Place the executable program in your folder of choice and get ready to run it.

#### 3. Create and Fill Configuration File
Create a `.env` file in the same folder where you put the PPDRIVE executable. Now copy all the [contents of this file](https://github.com/prodbyola/ppdrive/blob/main/.env.example) and put them in the newky created `.env` file.

#### 4. Modify Configuration
Now update the configuration file (`.env`) according to your project. Remember to use the database credentials you created earlier in your configuration. Please see [Configuration](/configuration) for more.

#### 5. Run PPDRIVE
Once your configuation is ready, run PPDRIVE.
```bash
./ppdrive
```
You should see a message like this:
```bash
ppdrive_app  | 2025-04-12T09:50:22.229817Z  INFO ppdrive: listening on 0.0.0.0:5000
```
Congratulations! The message indicates a PPDRIVE instance is running on port `5000` (or any other port you specified in [Configuration](/configuration)).


**NOTE: If you prefer, you can simply pull [PPDRIVE's Docker Image](https://hub.docker.com/repository/docker/prodbyola/ppdrive) and update the [Configuration](/configuration).**

# API Documentation
PPDRIVE exposes a simple and flexible REST API to manage your digital assets.

### Endpoints
Here are some key API endpoints:

- POST /assets/upload: Upload a new asset.

- GET /assets/{id}: Retrieve asset metadata.

- GET /assets/{id}/download: Download the asset.

- DELETE /assets/{id}: Delete an asset.

For a full list of available API endpoints and their usage, refer to the API Documentation.