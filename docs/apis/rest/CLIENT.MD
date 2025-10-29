# Client Mode APIs
#### Setting Up for Client Requests
This section shows how to connect to PPDRIVE in client mode. Before you start, please make sure a PPDRIVE service is running in [Client Mode](../../GETTING-STARTED.MD#21-client-mode) and you have the `ID` of the service. You can check running services with `ppdrive list` command and see `Auth-modes` column:

Once you confirm that a service is running in client mode and you have its `ID`, follow these steps:
1. Create a new client for the service:
```sh
ppdrive client create --svc-id [ID] --name [client_name]
```

  - `svc-id` is self-explanatory, right?
  - `name` is the name assigned to your client. Hint: Use a name you can easily associate with the client. For example, "Raymond Edutech App". Please note that `name` is required for creating a client but **not unique**.
  [img=../../assets/ppd_create_client.png]
  
  If you think your token is compromised, you can refresh it with:
```sh
ppdrive client refresh --svc-id [ID] --client-id [ID]
```

2. After the client is created, copy and save token **securely** in your client application. **<span color=red>TOKENS MUST BE KEPT SECRET!</span>** Do not expose your client token to public APIs. For example, you may save it in your environment variables:
```sh
PPD_TOKEN=12345678910...
```

3. Add the token to `ppd-client-token` header and send requests to [available endpoints](#api-endpoints).
```js
const ppdToken = process.env.PPD_TOKEN;

fetch({
    url: `${PPDRIVE_URL}/client/user`,
    headers: {
        "ppd-client-token": ppdToken
    }
})
```

#### API Endpoints
This are the endpoints **currently available** on PPDRIVE client mode:

###### Create/Register User
**Description**: Registers a new user for this client.
**Url:** {base_url}/client/user/register
**Method:** POST
**Headers:**
  ppd-client-token (_string_): The client's token
**Body:**
  