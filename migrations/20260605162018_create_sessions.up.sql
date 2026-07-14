CREATE TABLE sessions
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT UNIQUE,
    used       BOOLEAN  DEFAULT FALSE,
    created_at TEXT NOT NULL
)