# Getting Started With PPDRIVE Service
This document shows demonstrates how to start running a PPDRIVE service. But before we get started, let's take a quick look at how a PPDRIVE works. There two important options we must provide:

#### 1. Service Type
In order to run a PPDRIVE service, we must specify the **Service Type**. The Service Type is what tells PPDRIVE what protocol to use for serving your APIs. At the time of writing this documentation, we only support REST (or RESTful APIs) and this is the **default** choice. In future updates, we'll be adding support for gRPC, graphQL...etc. To run a service with a specific type, open your terminal and type:
```sh
ppdrive run [service_type]
```

For example, to run a REST service, you would type:
```sh
ppdrive run rest
```

#### 2. Auth Mode
In addition to specifying the Service Type, we also need to tell PPDRIVE what **Authentication Mode** (or auth mode) to run in. This will tell PPDRIVE how to protect your filesystem and authenticate incoming requests. What auth mode you choose **greatly depends on why and how you want to use PPDRIVE**. You can specify one or multiple auth modes. Here's how to specify auth modes(s) when starting a service:
```sh
ppdrive run [service_type] --auth-modes [auth_modes]

# Example (single auth mode)
ppdrive run rest --auth-modes client

# Example (multiple auth modes)
ppdrive run rest --auth-modes admin client
```
There are basically 4 auth modes and we'll quickly describe them. 

###### 2.1 Client Mode
Client mode allows you to connect to PPDRIVE by using a generated **client token**. The token must be provided by external applications in order to perform administrative tasks. This mode should be set if your goal is to **connect PPDRIVE to an application you're building seperately**. For example, you might be building an invoicing app and want to upload PDF files to PPDRIVE so you can share with customers. Client mode requires some extra steps. Please see [Setting Up for Client Mode](#setting-up-for-client-mode).

###### 2.2 Direct Mode
This mode tells PPDRIVE to allow users to interact _directly_ with your filesystem using their login credentials. You don't need a client token. This is the ideal choice if you're building a client application that **allows users to files/folders in an authenticated manner**. An example is when you're building a mobile app that works like Dropbox or Google Drive. You simply develop the user interface (UI) and interact directly with PPDRIVE API.

###### 2.3 Admin Mode
Allows you to take full control of PPDRIVE. However, access will only be granted to admin accounts.

###### 2.4 Zero Mode
No authentication is provided. Be careful not to set this mode accidentally as it will grant unrestricted access to users.

#### Setting Up for Client Mode
This section shows how to connect to PPDRIVE in client mode. Make sure you already started a service in client mode and you have the `id` of the service. You can check running services with:
```sh
ppdrive list
```

Now follow these steps to connect to your service.
1. Once you have your service `id`, create a client token:
```sh
ppdrive create-client [service_id] [token_name]
```

`service_id` is the is self-explanatory, right? `token_name` is the name assigned to your token. Hint: you may use the name of your client app.
2. Save token in your application.
3. Add token to `ppd-client-token` header and send requests.
