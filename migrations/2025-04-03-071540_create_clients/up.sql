-- Your SQL goes here
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE clients (
    id SERIAL PRIMARY KEY,
    enc_key BYTEA NOT NULL,
    enc_payload BYTEA NOT NULL,
    cid UUID UNIQUE NOT NULL DEFAULT uuid_generate_v4()
)