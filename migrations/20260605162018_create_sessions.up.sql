CREATE TABLE sessions
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT UNIQUE,
    used       BOOLEAN  DEFAULT FALSE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
)