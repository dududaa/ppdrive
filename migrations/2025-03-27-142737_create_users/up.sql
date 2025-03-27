-- Your SQL goes here
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    is_admin BOOLEAN DEFAULT FALSE NOT NULL,
    permission_group SMALLINT CHECK (permission_group BETWEEN 0 AND 255) NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP NOT NULL
);