CREATE TABLE asset_owner
(
    id   INTEGER PRIMARY KEY,
    name SMALLINT
);

CREATE TABLE clients
(
    id              INTEGER PRIMARY KEY,
    pid             TEXT UNIQUE,
    key             TEXT UNIQUE,
    name            TEXT    NOT NULL,
    max_bucket_size REAL,
    owner_id        INTEGER NOT NULL,
    created_at      TEXT    NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES asset_owner (id) ON DELETE CASCADE
);
CREATE INDEX idx_clients_asset_owner ON clients (owner_id);

CREATE TABLE users
(
    id         INTEGER PRIMARY KEY,
    email      TEXT    NOT NULL,
    password   TEXT    NOT NULL,
    owner_id   INTEGER NOT NULL,
    created_at TEXT    NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES asset_owner (id) ON DELETE CASCADE
);
CREATE INDEX idx_users_asset_owner on users (owner_id);

CREATE TABLE buckets
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT UNIQUE,
    size       REAL,
    accepts    TEXT,
    created_at TEXT    NOT NULL,
    owner_id   INTEGER NOT NULL,
    FOREIGN KEY (owner_id) REFERENCES asset_owner (id) ON DELETE CASCADE
);
CREATE INDEX idx_buckets_asset_owner on buckets (owner_id);
