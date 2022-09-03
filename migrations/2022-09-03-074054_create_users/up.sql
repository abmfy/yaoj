-- Your SQL goes here
CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    user_name TEXT NOT NULL UNIQUE
);

INSERT INTO users (id, user_name) VALUES (0, 'root');