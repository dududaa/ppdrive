# Client Mode APIs
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