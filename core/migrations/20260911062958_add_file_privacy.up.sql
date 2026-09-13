CREATE TABLE assets
(
    id         INTEGER PRIMARY KEY,
    pid        TEXT    UNIQUE NOT NULL,
    bucket_id  INTEGER NOT NULL,
    path       TEXT    NOT NULL,
    created_at TEXT    NOT NULL,
    FOREIGN KEY (bucket_id) REFERENCES buckets (id) ON DELETE CASCADE
);
CREATE INDEX idx_assets_bucket_id ON assets (bucket_id);
CREATE UNIQUE INDEX idx_assets_bucket_path ON assets (bucket_id, path);

CREATE TABLE file_permissions
(
    id         INTEGER PRIMARY KEY,
    asset_id   INTEGER NOT NULL,
    grantee_id INTEGER NOT NULL,
    permission SMALLINT NOT NULL DEFAULT 0,
    created_at TEXT    NOT NULL,
    FOREIGN KEY (asset_id) REFERENCES assets (id) ON DELETE CASCADE,
    FOREIGN KEY (grantee_id) REFERENCES asset_owner (id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX idx_file_permissions_asset_grantee ON file_permissions (asset_id, grantee_id);
