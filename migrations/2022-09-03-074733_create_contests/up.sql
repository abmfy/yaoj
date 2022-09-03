-- Your SQL goes here
CREATE TABLE contests (
    id INTEGER PRIMARY KEY NOT NULL,
    contest_name TEXT NOT NULL,
    contest_from DATETIME NOT NULL,
    contest_to DATETIME NOT NULL,
    problem_ids TEXT NOT NULL,
    user_ids TEXT NOT NULL,
    submission_limit INTEGER NOT NULL
)