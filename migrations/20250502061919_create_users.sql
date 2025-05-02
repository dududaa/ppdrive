CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS users
(
    id SERIAL PRIMARY KEY,
    pid UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),
    permission_group SMALLINT CHECK (permission_group BETWEEN 0 AND 255) NOT NULL,
    root_folder VARCHAR(200),
    folder_max_size BIGiNT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);
