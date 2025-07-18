## CONNECTING PPDRIVE
PPDRIVE provides two ways to access private resources, depending on where you want to use PPDRIVE directly or connect to an exisiting application. Let's explore the 2 access types:

#### 1. Client Access
This access type allows you to connect your **existing application** to PPDRIVE.
- First, you need to create a PPDRIVE `Client` by running this in your terminal:
```sh
ppdrive create-client {client_name}
```
- Then, copy and save the client token securely in your application server.
- Now pass the token to any client request that requires administrative operations (create user, login user...etc)

###### Authentication and Authorization
Client Access authenticates and authorizes users in 2 ways:
1. **External Authentication**: You can provide your own external authentication url as `auth.url` in PPDRIVE configuration file. We currently support JWT auth and we'll check if the request has an authorization header, which we then pass to the request we make to `auth.url`. Your auth url **must** be a POST that returns a valid PPDRIVE user `pid`, provided by PPDRIVE when the user was created.
2. **Host Authentication**: This is the default authentication method if you do not provide a `auth.url`. The authentication is performed directly by PPDRIVE. You can get your access tokens by visiting `POST: /client/user/login`. The endpoint will return both access and refresh tokens, each with expirations as set in `auth.access_exp` and `auth.refresh_exp` respectively.

#### 2. Direct Access
If you're trying to build your own file manager, cloud storage or anything that doesn't require a `Client`, PPDRIVE gives you direct access with **Host Authentication** as decribed above. No need to create a `Client`.
- Visit `POST: /user/register` a user.
- Visit `POST: /user/login` for **Host Authentication**.

---

## BUCKETS
Buckets is how we manage and protect asset management(file upload, folder creation...etc).
- Each asset (file or folder) must added to a bucket.
- A bucket may be created by a `Client` or directly by a user.
- A user, if allowed, has a maximum bucket they can create.
- Bucket can be public or private. A public bucket is writable by everyone. A private bucket is writable to only its owner and users with access.
- When writing to bucket, we check if user is bucket owner. If not, we check if it's a public bucket. If not, the operation fails.
