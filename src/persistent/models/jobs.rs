use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;

use serde::Deserialize;

use crate::api::err::{Error, Reason};
use crate::api::jobs::{CaseResults, JobResult, JobStatus};
use crate::persistent::schema::jobs;

#[derive(Clone, Queryable, Insertable, AsChangeset, Identifiable)]
pub struct Job {
    pub id: i32,
    pub created_time: NaiveDateTime,
    pub updated_time: NaiveDateTime,
    pub source_code: String,
    pub lang: String,
    pub user_id: i32,
    pub contest_id: i32,
    pub problem_id: i32,
    pub job_state: JobStatus,
    pub result: JobResult,
    pub score: f64,
    pub cases: CaseResults,
}

/// We need to convert between api::jobs::Job and persistent::models::Job
/// because the response need to be serialized to json with
/// some deeper nested structure (the submission substructure), white
/// the latter need to be flatten to be stored in the database
impl From<crate::api::jobs::Job> for Job {
    fn from(job: crate::api::jobs::Job) -> Self {
        Job {
            id: job.id as i32,
            created_time: job.created_time.naive_utc(),
            updated_time: job.updated_time.naive_utc(),
            source_code: job.submission.source_code,
            lang: job.submission.language,
            user_id: job.submission.user_id as i32,
            contest_id: job.submission.contest_id as i32,
            problem_id: job.submission.problem_id as i32,
            job_state: job.state,
            result: job.result,
            score: job.score,
            cases: CaseResults(job.cases),
        }
    }
}

#[derive(Default, Deserialize)]
pub struct JobFilter {
    pub user_id: Option<i32>,
    pub user_name: Option<String>,
    pub contest_id: Option<i32>,
    pub problem_id: Option<i32>,
    pub language: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub state: Option<JobStatus>,
    pub result: Option<JobResult>,
}

/// Returns if a specific job exists
pub fn does_job_exist(conn: &mut SqliteConnection, jid: i32) -> Result<bool, Error> {
    use self::jobs::dsl::*;

    let job = jobs.find(jid).first::<Job>(conn).optional()?;

    Ok(job.is_some())
}

/// Returns the count of jobs
pub fn jobs_count(conn: &mut SqliteConnection) -> Result<i32, Error> {
    use self::jobs::dsl::*;

    let count: i64 = jobs.count().get_result(conn)?;

    Ok(count as i32)
}

/// Add a new job to the database
pub fn new_job(conn: &mut SqliteConnection, job_form: Job) -> Result<Job, Error> {
    use self::jobs::dsl::*;

    Ok(diesel::insert_into(jobs)
        .values(job_form)
        .get_result(conn)?)
}

/// Get specific job
pub fn get_job(conn: &mut SqliteConnection, jid: i32) -> Result<Job, Error> {
    use self::jobs::dsl::*;

    jobs.find(jid)
        .first(conn)
        .optional()?
        .ok_or_else(|| Error::new(Reason::NotFound, format!("Job {} not found.", jid)))
}

/// Get filtered jobs
pub fn get_jobs(conn: &mut SqliteConnection, filt: JobFilter) -> Result<Vec<Job>, Error> {
    use self::jobs::dsl::*;

    // Construct query conditions from JobFilter
    let mut query = jobs.into_boxed();
    if let Some(uid) = filt.user_id {
        query = query.filter(user_id.eq(uid));
    }
    if let Some(username) = filt.user_name {
        let uid = super::users::get_id_by_username(conn, &username)?.unwrap_or(-1);
        query = query.filter(user_id.eq(uid));
    }
    if let Some(cid) = filt.contest_id {
        query = query.filter(contest_id.eq(cid));
    }
    if let Some(pid) = filt.problem_id {
        query = query.filter(problem_id.eq(pid));
    }
    if let Some(language) = filt.language {
        query = query.filter(lang.eq(language));
    }
    if let Some(from) = filt.from {
        query = query.filter(created_time.ge(from.naive_utc()));
    }
    if let Some(to) = filt.to {
        query = query.filter(created_time.le(to.naive_utc()));
    }
    if let Some(state) = filt.state {
        query = query.filter(job_state.eq(state));
    }
    if let Some(res) = filt.result {
        query = query.filter(result.eq(res));
    }

    Ok(query.load(conn)?)
}

/// Get the latest submission of a user on a problem in a contest
pub fn get_latest_submission(
    conn: &mut SqliteConnection,
    uid: i32,
    pid: i32,
    cid: i32,
) -> Result<Option<Job>, Error> {
    use self::jobs::dsl::*;

    Ok(jobs
        .filter(user_id.eq(uid))
        .filter(problem_id.eq(pid))
        .filter(contest_id.eq(cid))
        .order(created_time.desc())
        .first(conn)
        .optional()?)
}

/// Get the submission which score is highest of a user on a problem in a contest
pub fn get_highest_submission(
    conn: &mut SqliteConnection,
    uid: i32,
    pid: i32,
    cid: i32,
) -> Result<Option<Job>, Error> {
    use self::jobs::dsl::*;

    Ok(jobs
        .filter(user_id.eq(uid))
        .filter(problem_id.eq(pid))
        .filter(contest_id.eq(cid))
        .order((score.desc(), created_time))
        .first(conn)
        .optional()?)
}

/// Get the count of submissions on a problem of a user in a contest
pub fn get_submission_count(
    conn: &mut SqliteConnection,
    uid: i32,
    pid: i32,
    cid: i32,
) -> Result<i64, Error> {
    use self::jobs::dsl::*;

    Ok(jobs
        .filter(user_id.eq(uid))
        .filter(problem_id.eq(pid))
        .filter(contest_id.eq(cid))
        .count()
        .get_result(conn)?)
}

/// Update an existing job
pub fn update_job(conn: &mut SqliteConnection, job_form: Job) -> Result<Job, Error> {
    Ok(job_form.save_changes(conn)?)
}
