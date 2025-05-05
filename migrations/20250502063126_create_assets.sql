CREATE TABLE IF NOT EXISTS assets(
    asset_path VARCHAR(3000) NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    public BOOLEAN DEFAULT FALSE NOT NULL,
    PRIMARY KEY (asset_path, user_id)
)
