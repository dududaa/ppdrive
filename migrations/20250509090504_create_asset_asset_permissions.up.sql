CREATE TABLE IF NOT EXISTS asset_permissions (
    user_id INTEGER NOT NULL,
    asset_id INTEGER NOT NULL,
    permission SMALLINT CHECK (permission BETWEEN 0 AND 255) NOT NULL
);

CREATE INDEX idx_user_asset ON asset_permissions (user_id, asset_id);
