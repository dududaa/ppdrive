-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE clients (
    id SERIAL PRIMARY KEY,
    public_key BYTEA NOT NULL,
    payload BYTEA NOT NULL,
    client_id UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4()
)