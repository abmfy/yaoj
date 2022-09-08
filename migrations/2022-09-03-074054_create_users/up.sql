-- Your SQL goes here
CREATE TABLE users (
    id INTEGER PRIMARY KEY NOT NULL,
    user_role INTEGER NOT NULL DEFAULT '0',
    user_name TEXT NOT NULL UNIQUE,
    passwd TEXT NOT NULL DEFAULT '09080453'
);

INSERT INTO users (id, user_role, user_name, passwd) VALUES (0, 2, 'root', '#!/*<!--*#*SUPER_SECRET_PASSWORD*#*-->*/')