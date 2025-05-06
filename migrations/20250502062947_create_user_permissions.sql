CREATE TABLE IF NOT EXISTS user_permissions(
    user_id INTEGER NOT NULL,
    permission SMALLINT CHECK (permission BETWEEN 0 AND 255) NOT NULL
);

CREATE INDEX idx_user_permissions ON user_permissions(user_id);
