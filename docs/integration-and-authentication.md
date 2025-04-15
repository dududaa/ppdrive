# Integration

Now that you've [installed PPDRIVE]('/'), it's time to integrate it into your exisiting application(s). Introducing Clients!

PPDRIVE Clients are external applications that connect with PPDRIVE to perform administrative tasks such as creating and managing users, managing permissions, rotating keys and more. They can do all these via the [admin routes]('/routes#admin'). For your application to connect with PPDRIVE through these routes, you need to register it as a recognized **Client**. Follow the steps below to complete the registration process:

#### 1. Generate API keys

PPDRIVE provides a _keygen_ tool for generating client keys. To create a new client, run the keygen tool:

```bash
./ppdrive keygen
```

**NOTE:** <span style="color: red">If your PPDRIVE instance is running on Docker, you should first access your container's terminal with Compose's command `docker compose exec ppdrive sh`</span>.

After running the keygen tool, PDDRIVE will create a **Client** and show you the client's API keys like these (real keys obfuscated):

```bash
Token generated success herefully!

PPD_PUBLIC: **********************
PPD_PRIVATE: *************************
CLIENT_ID: ****************************
```

You'll need to save these keys securely and pass them to your application(s). We'll show you how to use the keys in next steps.

#### 2. Pass the keys to your client's requests

Once the API keys have been generated, store them securely in a location your application can access—such as environment variables. When your application needs to call an [admin route](/routes#admin), include the keys in the request headers: `X-API-KEY` and `X-CLIENT-ID`. Below is an example using JavaScript (NodeJs) with [Axios](https://axios-http.com/docs/intro) but you can implement this in any language:

```javascript
// `ppdriveUrl` is where PPDRIVE is deployed
const url = ppdriveUrl+"/admin/user"

// options for creating user
const opts = {...}

// Get API keys from environment variables
const clientId = process.env.CLIENT_ID;
const publicKey = process.env.PPD_PUBLIC;
const privateKey = process.env.PPD_PRIVATE;
const apiKey = publicKey + "." + privateKey // combine public and private keys

const config = {
    headers: {
        'X-CLIENT-ID': clientId,
        'X-API-KEY': apiKey
    }
}

axios.post(url, opts, config)
```

The code above will send a "create new user" request to PPDRIVE. Notice how we concatenate the `public` and `private` PPDRIVE keys using a dot "."?. The request will be successful only if `X-CLIENT-ID` and `X-API-KEY` are present in the headers and only if they contain valid keys.

# Authentication
PPDRIVE manages its own records of application users in order to understand and track which assets belongs to who, securing user assets from unauthorized access. At the moment, PPDRIVE depends on your (external) app for request authentication. We're working on adding independent authentication mechanisms for those who desire. For now, you have to set PPDRIVE_AUTH_URL to tell PPDRIVE where to go for users authentication. Don't worry if you don't understand just yet. This guide will show you how everything fits together.

For users to upload assets and access other secured routes on PPDRIVE, a user account must be created (and