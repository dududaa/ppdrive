CREATE TABLE asset_owner
(
    id       INTEGER PRIMARY KEY,
    name     SMALLINT,
    owner_id INTEGER NOT NULL
);

CREATE UNIQUE INDEX idx_asset_owner_name_id ON asset_owner (name, owner_id);

CREATE TABLE clients
(
    id              INTEGER PRIMARY KEY,
    pid             TEXT UNIQUE,
    key             TEXT UNIQUE,
    name            TEXT NOT NULL,
    max_bucket_size REAL,
    created_at      TEXT NOT NULL
);

CREATE TABLE users
(
    id         INTEGER PRIMARY KEY,
    email      TEXT UNIQUE NOT NULL,
    password   TEXT        NOT NULL,
    created_at TEXT        NOT NULL
);

CREATE TABLE buckets
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT UNIQUE,
    size       BIGINT,
    accepts    TEXT,
    created_at TEXT    NOT NULL,
    owner_id   INTEGER NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES asset_owner (id) ON DELETE CASCADE
);
CREATE INDEX idx_buckets_asset_owner on buckets (owner_id);
