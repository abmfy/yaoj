use chrono::{DateTime, NaiveDateTime, Utc};
use diesel::prelude::*;

use serde::Deserialize;

use crate::api::err::{Error, Reason};
use crate::api::jobs::{CaseResults, JobResult, JobStatus};
use crate::persistent::schema::jobs;

#[derive(Queryable)]
pub struct Job {
    id: i32,
    created_time: NaiveDateTime,
    updated_time: NaiveDateTime,
    source_code: String,
    lang: String,
    user_id: i32,
    contest_id: i32,
    problem_id: i32,
    job_state: JobStatus,
    result: JobResult,
    score: f64,
    cases: CaseResults,
}

#[derive(Insertable, AsChangeset)]
#[diesel(table_name = jobs)]
pub struct JobForm {
    created_time: NaiveDateTime,
    updated_time: NaiveDateTime,
    source_code: String,
    lang: String,
    user_id: i32,
    contest_id: i32,
    problem_id: i32,
    job_state: JobStatus,
    result: JobResult,
    score: f64,
    cases: CaseResults,
}

#[derive(Deserialize)]
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
pub fn does_job_exist(conn: &mut SqliteConnection, id: i32) -> Result<bool, Error> {
    use self::jobs::dsl::*;

    let job = jobs
        .find(id)
        .first::<Job>(conn)
        .optional()?;

    Ok(job.is_some())
}

/// Add a new job to the database
pub fn new_job(conn: &mut SqliteConnection, job_form: JobForm) -> Result<Job, Error> {
    use self::jobs::dsl::*;

    Ok(diesel::insert_into(jobs)
        .values(job_form)
        .get_result(conn)?)
}

/// Get specific job
pub fn get_job(conn: &mut SqliteConnection, jid: i32) -> Result<Job, Error> {
    use self::jobs::dsl::*;

    jobs.find(id).first(conn).optional()?.ok_or(Error::new(
        Reason::NotFound,
        format!("Job {} not found.", jid),
    ))
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

    Ok(vec![])
}

/// Update an existing job
pub fn update_job(conn: &mut SqliteConnection, jid: i32, job_form: JobForm) -> Result<Job, Error> {
    use self::jobs::dsl::*;

    Ok(diesel::update(jobs.find(jid))
        .set(job_form)
        .get_result(conn)?)
}
