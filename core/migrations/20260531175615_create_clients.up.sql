CREATE TABLE asset_owner
(
    id       INTEGER PRIMARY KEY,
    name     SMALLINT,
    owner_id INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_asset_owner_name_id ON asset_owner (name, owner_id);

CREATE TABLE clients
(
    id            INTEGER PRIMARY KEY,
    pid           TEXT UNIQUE,
    encrypted_key TEXT NOT NULL,
    key_nonce     BYTEA NOT NULL,
    key_hash      TEXT NOT NULL UNIQUE,
    name          TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

CREATE TABLE users
(
    id         INTEGER PRIMARY KEY,
    email      TEXT UNIQUE NOT NULL,
    password   TEXT        NOT NULL,
    metadata   TEXT,
    is_admin   SMALLINT     NOT NULL DEFAULT 0,
    created_at TEXT        NOT NULL,
    updated_at TEXT        NOT NULL
);

CREATE TABLE buckets
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT UNIQUE,
    name       TEXT     NOT NULL,
    public     SMALLINT  NOT NULL DEFAULT 0,
    size       BIGINT,
    accepts    TEXT,
    path       TEXT     NOT NULL UNIQUE,
    created_at TEXT     NOT NULL,
    owner_id   INTEGER  NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES asset_owner (id) ON DELETE CASCADE
);
CREATE INDEX idx_buckets_asset_owner on buckets (owner_id);
CREATE INDEX idx_buckets_public on buckets (public);
