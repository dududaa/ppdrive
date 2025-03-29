-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    pid UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4(),
    is_admin BOOLEAN DEFAULT FALSE NOT NULL,
    permission_group SMALLINT CHECK (permission_group BETWEEN 0 AND 255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);