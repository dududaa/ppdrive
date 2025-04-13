# Configuration
PPDRIVE offers flexible configuration options. You can configure it by either creating a .env file in the same directory where PPDRIVE is installed or by setting the values directly in your environment variables. For guidance, refer to the provided [.env.example](https://github.com/prodbyola/ppdrive/blob/main/.env.example) file — you can copy it or use it as a template. 

**NOTE: PPDRIVE uses [Postgresql](https://www.postgresql.org/) database for managing records internally.**

### PPDRIVE_DB_USER
Database user

### PPDRIVE_DB_PASSWORD
Database password

### PPDRIVE_DB_NAME
Name of database to use

### PPDRIVE_DB_PORT
By default, Postgresql users port `5432`. However if you're [installing PPDRIVE with Docker Compose](/?id=installation-with-docker-recommended), this default port exposed by `ppdrive_db` service might be in use by another Postgres instance (or any other program) on your host machine, which can lead to conflicts. If you encountered any issue like `5432` port conflict, set `PPDRIVE_DB_PORT` option to a different port other than `5432`.

### PPDRIVE_AUTH_URL
An external authorization endpoint used by PPDRIVE to authenticate users. See [Authentication] for more details.

### PPDRIVE_PORT
Port to be used by PPDRIVE. Must be an available port on your machine. Change this if port `5000` is not available.

### DATABASE_URL
Database URL used internally by PPDRIVE. Defaults to `postgres://${PPDRIVE_DB_USER}:${PPDRIVE_DB_PASSWORD}@127.0.0.1:${PPDRIVE_DB_PORT}/${PPDRIVE_DB_NAME}`. Change this if your Postgres instance is running on a different host.

### PPDRIVE_ALLOWED_ORIGINS
PPDRIVE implements [Cors policy](https://aws.amazon.com/what-is/cross-origin-resource-sharing/#:~:text=Cross%2Dorigin%20resource%20sharing%20(CORS)%20is%20an%20extension%20of,that%20are%20public%20or%20authorized.) to limit origin access. If your website/client is prevented by cors, whitelist your website(s) by configuring this option.

- For single website:
```bash
PPDRIVE_ALLOWED_ORIGINS=https://myclientapp.com
```

- For multiple websites (separated with comma):
```bash
PPDRIVE_ALLOWED_ORIGINS=https://myclientapp1.com,https://myclientapp2.com
```

- For all websites:
```bash
PPDRIVE_ALLOWED_ORIGINS=*
```

DEBUG_MODE=true # should be false in production