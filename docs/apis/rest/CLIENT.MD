# Client Mode APIs
## Setting Up for Client Requests
This section shows how to connect to PPDRIVE in client mode. Before you start, please make sure a PPDRIVE service is running in [Client Mode](../../GETTING-STARTED.MD#21-client-mode) and you have the `ID` of the service. You can check running services with `ppdrive list` command and see `Auth-modes` column:

![PPDrive Client List](../../assets/ppdrive_svc_list.png)

Once you confirm that a service is running in client mode and you have its `ID`, follow these steps:
1. Create a new client for the service:
```sh
ppdrive client create --svc-id [ID] --name [client_name]
```

  - `svc-id` is self-explanatory, right?
  - `name` is the name assigned to your client. Hint: Use a name you can easily associate with the client. For example, "Raymond Edutech App". Please note that `name` is required for creating a client but **not unique**.

  If the client is successfully created, the client's `id` and secret `token` would be printed like this:

  ![PPDrive Create Client](../../assets/ppd_create_client.png)

2. After the client is created, copy and save token **securely** in your client application **<span color=red>TOKENS MUST BE KEPT SECRET!</span>** Do not expose your client token to public APIs. For example, you may save it in your environment variables:
```sh
PPD_TOKEN=74352d4a4df...
```

  If you think your token is compromised, you can refresh it with:
  ```sh
  ppdrive client refresh --svc-id [ID] --client-id [ID]
  ```

3. Add the token to `ppd-client-token` header and send requests to [available endpoints](#api-endpoints).
```js
const ppdToken = process.env.PPD_TOKEN;
const baseUrl = "0.0.0.0:5000" // or whatever location PPDRIVE is running from.

const resp = await fetch(`${baseUrl}/client/user`, {
    headers: {
        'Content-Type': 'application/json',
        'ppd-client-token': ppdToken
    }
})
```

## API Endpoints
These are the endpoints **currently available** on PPDRIVE client mode:

###### User Registration
**Endpoint:** /client/user/register

**Description**: Registers a new user for this client.

**Method:** POST

**Headers:**
  - `ppd-client-token` (_string_): The client's token

**Body:**
  - `max_bucket`:
    - required: `false`
    - type: `float64`
    - description: Total size of buckets the user can create in megabytes (MB). This is the total accumulated size, which means the user can create as many buckets as possible as long as the total size of all the buckets combined doesn't exceed this size. To put simply, this is **total filesize** this user can upload.
    
      When the option is not specified, user can create unlimited number of buckets and in turn, unlimited files.

**Response:**
  - type: `string`
  - description: Public ID (UUID) of the user created.
  
**Example:**
```js
const resp = await fetch(`${baseUrl}/client/user/register`, {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'ppd-client-token': ppdToken
    },
    body: JSON.stringify({
      max_bucket: 10.0 // 10MB space assigned to the user
    })
})
```