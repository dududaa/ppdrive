# Getting Started with PPDRIVE Service
The quickest way to spawn a PPDRIVE service is to simply run:
```sh
ppdrive launch rest
```
This will attept to start a PPDRIVE **REST** service in [client mode](#21-client-mode) on port `5000`. If the port is not available, you can try a different port:
```sh
ppdrive launch rest --port [port-number]
```

Once, the service is running, you should check out [client mode APIs](/docs/apis/rest/CLIENT.MD) to start uploading and managing files from your application.

However, to have a better understanding of different service types, authentication modes and database setup, we highly recommend you read the rest of this document.


## 1. Service and Service Type
A **Service** is a backend process that PPDRIVES runs to expose different APIs for managing the host filesystem. By default, when we launch a service, PPDRIVE spawns a new process on port `5000`. It throws an error if the port is already in use, in which case we can specify a different port with the `--port` option. 

To run a PPDRIVE service, we must specify the **Service Type**. The Service Type is what tells PPDRIVE what protocol we're serving our APIs on. At the time of writing this documentation, PPDRIVE only supports REST (or RESTful APIs) and this is the **default** choice. In future updates, we'll be adding support for gRPC, graphQL...etc. To run a service with a specific type, open your terminal and run:
```sh
ppdrive launch [service_type]
```

For example, to run a REST service, you would type:
```sh
ppdrive launch rest
```

## 2. Auth Mode
In addition to specifying the Service Type, we also need to tell PPDRIVE what **Authentication Mode** (or auth mode) to use. This way, PPDRIVE knows how to protect your filesystem and authenticate incoming requests. What auth mode you choose **greatly depends on why and how you want to use PPDRIVE**. You can specify one or multiple auth modes:
```sh
ppdrive launch [service_type] --auth-modes [auth_mode_1,auth_mode_2, ...]

# Example (single auth mode)
ppdrive launch rest --auth-modes client

# Example (multiple auth modes)
ppdrive launch rest --auth-modes admin,client
```
There are basically 4 auth modes: _client_, _direct_, _admin_ and _zero_ mode. Let's quickly describe each of them. 

#### 2.1 Client Mode
Client mode allows you to connect to PPDRIVE by using a generated **client token**. The token must be provided by external applications in order to perform administrative tasks. This mode should be set if your goal is to **connect PPDRIVE to an application you're building seperately**. For example, you might be building an invoicing app and want to upload PDF files to PPDRIVE so you can share with customers. Client mode requires some extra steps. Please see [Setting Up for Client Mode](#setting-up-for-client-mode).

#### 2.2 Direct Mode
This mode tells PPDRIVE to allow users to interact _directly_ with your filesystem using their login credentials. You don't need a client token. This is the ideal choice if you're building a client application that **allows users to files/folders in an authenticated manner**. An example is when you're building a mobile app that works like Dropbox or Google Drive. You simply develop the user interface (UI) and interact directly with PPDRIVE API.

#### 2.3 Admin Mode
Allows you to take full control of PPDRIVE. However, access will only be granted to admin accounts. You'll be able to manage, other admins, users, database and all PPDRIVE components. 

#### 2.4 Zero Mode
No authentication is provided. Be careful not to set this mode accidentally as it will grant unrestricted access to users.


## 3. Database
**Note:** All auth modes except _zero_ require a database connection.

PPDRIVE uses a database to manage its own record of users, filesystem and other activities. At the moment, we support 3 database backends: SQLite (default), Postgres and MySQL. The `--database` option allows you to tell which database url to use:
```sh
ppdrive launch rest --database sqlite://db.sqlite
```

The default value is a SQLite file: `sqlite://db.sqlite`.