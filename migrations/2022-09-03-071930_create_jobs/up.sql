-- Your SQL goes here
CREATE TABLE jobs (
    id INTEGER PRIMARY KEY NOT NULL,
    created_time DATETIME NOT NULL,
    updated_time DATETIME NOT NULL,
    source_code TEXT NOT NULL,
    lang TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    contest_id INTEGER NOT NULL,
    problem_id INTEGER NOT NULL,
    job_state INTEGER NOT NULL,
    result INTEGER NOT NULL,
    score DOUBLE NOT NULL,
    cases TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id),
    FOREIGN KEY(contest_id) REFERENCES contests(id)
)