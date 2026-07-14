CREATE TABLE clients
(
    id    INTEGER PRIMARY KEY,
    pid   TEXT UNIQUE,
    key TEXT UNIQUE,
    name TEXT NOT NULL,
    max_bucket_size REAL,
    created_at TEXT NOT NULL
)