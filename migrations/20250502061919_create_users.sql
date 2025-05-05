CREATE TABLE IF NOT EXISTS users
(
    id SERIAL PRIMARY KEY,
    pid VARCHAR(36) UNIQUE NOT NULL,
    permission_group SMALLINT CHECK (permission_group BETWEEN 0 AND 255) NOT NULL,
    root_folder VARCHAR(200),
    folder_max_size BIGiNT,
    created_at VARCHAR(120) NOT NULL
);
