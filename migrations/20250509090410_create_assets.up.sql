CREATE TABLE IF NOT EXISTS assets (
    id SERIAL PRIMARY KEY,
    asset_path VARCHAR(3000) NOT NULL UNIQUE,
    user_id INTEGER NOT NULL,
    public BOOLEAN DEFAULT FALSE NOT NULL,
    custom_path VARCHAR(120),
    UNIQUE (user_id, asset_path)
)
