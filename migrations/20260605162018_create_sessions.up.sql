CREATE TABLE sessions
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT UNIQUE,
    used       BOOLEAN DEFAULT FALSE,
    info       TEXT,
    created_at TEXT NOT NULL
)