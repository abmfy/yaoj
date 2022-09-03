use actix_web::{
    get, post, put,
    web::{self, Data, Json, Path, Query},
};
use chrono::{DateTime, Utc};
use diesel::{
    backend::{self, Backend},
    deserialize::FromSql,
    serialize::{IsNull, Output, ToSql},
    sql_types::{Integer, Text},
    sqlite::Sqlite,
    AsExpression, FromSqlRow,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::err::{Error, Reason};

use crate::{persistent::models, DbPool};

use crate::{config::Config, judge::judge};

#[derive(Clone, Serialize, Deserialize)]
pub struct Submission {
    pub source_code: String,
    pub language: String,
    pub user_id: i32,
    pub contest_id: i32,
    pub problem_id: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[diesel(sql_type = Integer)]
pub enum JobStatus {
    Queueing,
    Running,
    Finished,
    Canceled,
}

impl ToSql<Integer, Sqlite> for JobStatus
where
    i32: ToSql<Integer, Sqlite>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(*self as i32);
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Integer, DB> for JobStatus
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: backend::RawValue<DB>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            0 => Ok(JobStatus::Queueing),
            1 => Ok(JobStatus::Running),
            2 => Ok(JobStatus::Finished),
            3 => Ok(JobStatus::Canceled),
            x => Err(format!("Unrecognized enum variant {x}").into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[diesel(sql_type = Integer)]
pub enum JobResult {
    Waiting,
    Running,
    Accepted,
    #[serde(rename = "Compilation Error")]
    CompilationError,
    #[serde(rename = "Compilation Success")]
    CompilationSuccess,
    #[serde(rename = "Wrong Answer")]
    WrongAnswer,
    #[serde(rename = "Runtime Error")]
    RuntimeError,
    #[serde(rename = "Time Limit Exceeded")]
    TimeLimitExceeded,
    #[serde(rename = "Memory Limit Exceeded")]
    MemoryLimitExceeded,
    #[serde(rename = "System Error")]
    SystemError,
    #[serde(rename = "SPJ Error")]
    SpjError,
    Skipped,
}

impl ToSql<Integer, Sqlite> for JobResult
where
    i32: ToSql<Integer, Sqlite>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(*self as i32);
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Integer, DB> for JobResult
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: backend::RawValue<DB>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            0 => Ok(JobResult::Waiting),
            1 => Ok(JobResult::Running),
            2 => Ok(JobResult::Accepted),
            3 => Ok(JobResult::CompilationError),
            4 => Ok(JobResult::CompilationSuccess),
            5 => Ok(JobResult::WrongAnswer),
            6 => Ok(JobResult::RuntimeError),
            7 => Ok(JobResult::TimeLimitExceeded),
            8 => Ok(JobResult::MemoryLimitExceeded),
            9 => Ok(JobResult::SystemError),
            10 => Ok(JobResult::SpjError),
            11 => Ok(JobResult::Skipped),
            x => Err(format!("Unrecognized enum variant {x}").into()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CaseResult {
    pub id: u32,
    pub result: JobResult,
    pub time: u32,
    pub memory: u32,
}

#[derive(Clone, Debug, AsExpression, FromSqlRow)]
#[diesel(sql_type = Text)]
pub struct CaseResults(pub Vec<CaseResult>);

impl ToSql<Text, Sqlite> for CaseResults
where
    String: ToSql<Text, Sqlite>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(json!(self.0).to_string());
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Text, DB> for CaseResults
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    fn from_sql(bytes: backend::RawValue<DB>) -> diesel::deserialize::Result<Self> {
        let s = String::from_sql(bytes)?;
        let v = serde_json::from_str(&s)?;
        Ok(CaseResults(v))
    }
}

#[derive(Clone, Serialize)]
pub struct Job {
    pub id: i32,
    pub created_time: DateTime<Utc>,
    pub updated_time: DateTime<Utc>,
    pub submission: Submission,
    pub state: JobStatus,
    pub result: JobResult,
    pub score: f64,
    pub cases: Vec<CaseResult>,
}

impl From<models::Job> for Job {
    fn from(job: models::Job) -> Self {
        Self {
            id: job.id,
            created_time: job.created_time.and_local_timezone(Utc).unwrap(),
            updated_time: job.updated_time.and_local_timezone(Utc).unwrap(),
            submission: Submission {
                source_code: job.source_code,
                language: job.lang,
                user_id: job.user_id,
                contest_id: job.contest_id,
                problem_id: job.problem_id,
            },
            state: job.job_state,
            result: job.result,
            score: job.score,
            cases: job.cases.0,
        }
    }
}

#[post("/jobs")]
/// Create a new submission
pub async fn new_job(
    submission: Json<Submission>,
    config: Data<Config>,
    pool: Data<DbPool>,
) -> Result<Json<Job>, Error> {
    const TARGET: &str = "POST /jobs";
    log::info!(target: TARGET, "Request received");

    let conn = &mut web::block(move || pool.get()).await??;

    match config.get_lang(&submission.language) {
        None => {
            log::info!(target: TARGET, "No such language: {}", submission.language);
            Err(Error::new(
                Reason::NotFound,
                format!("No such language: {}", submission.language),
            ))
        }
        Some(lang) => {
            match config.get_problem(submission.problem_id) {
                None => {
                    log::info!(target: TARGET, "No such problem: {}", submission.problem_id);
                    Err(Error::new(
                        Reason::NotFound,
                        format!("No such problem: {}", submission.problem_id),
                    ))
                }
                Some(problem) => {
                    let uid = submission.user_id;
                    let user_exists = models::does_user_exist(conn, uid)?;
                    if !user_exists {
                        log::info!(target: TARGET, "No such user: {}", submission.user_id);
                        return Err(Error::new(
                            Reason::NotFound,
                            format!("No such user: {}", submission.user_id),
                        ));
                    }

                    let created = Utc::now().naive_utc();

                    // Add the job to the jobs list with Running status
                    let job = models::Job {
                        id: models::jobs_count(conn)?,
                        created_time: created,
                        updated_time: created,
                        source_code: submission.source_code.clone(),
                        lang: submission.language.clone(),
                        user_id: submission.user_id,
                        contest_id: submission.contest_id,
                        problem_id: submission.problem_id,
                        job_state: JobStatus::Running,
                        result: JobResult::Waiting,
                        score: 0.0,
                        cases: CaseResults(vec![]),
                    };
                    let job_id = models::new_job(conn, job.clone())?.id;
                    log::info!(target: TARGET, "Job {} created", job_id);

                    log::info!(target: TARGET, "Judging started");
                    let result = judge(&submission.source_code, lang, problem);
                    log::info!(target: TARGET, "Judging ended, result: {:?}", result.result);
                    // Add the job to the jobs list
                    let job = models::Job {
                        updated_time: Utc::now().naive_utc(),
                        job_state: JobStatus::Finished,
                        result: result.result,
                        score: result.score,
                        cases: CaseResults(result.cases),
                        ..job
                    };
                    let job = models::update_job(conn, job)?;
                    log::info!(target: TARGET, "Job {} added", job_id);
                    log::info!(target: TARGET, "Request done");
                    Ok(Json(job.into()))
                }
            }
        }
    }
}

type JobFilter = models::JobFilter;

#[get("/jobs")]
pub async fn get_jobs(
    filter: Query<JobFilter>,
    pool: Data<DbPool>,
) -> Result<Json<Vec<Job>>, Error> {
    const TARGET: &str = "GET /jobs";
    log::info!(target: TARGET, "Request received");

    let filtered_jobs = web::block(move || {
        let mut conn = pool.get()?;
        models::get_jobs(&mut conn, filter.into_inner())
    })
    .await??;

    log::info!(target: TARGET, "Request done");
    Ok(Json(
        filtered_jobs.into_iter().map(|job| job.into()).collect(),
    ))
}

#[get("/jobs/{id}")]
pub async fn get_job(id: Path<i32>, pool: Data<DbPool>) -> Result<Json<Job>, Error> {
    const TARGET: &str = "GET /jobs/{id}";
    log::info!(target: TARGET, "Request received");

    let id = id.into_inner();
    let job = web::block(move || {
        let mut conn = pool.get()?;
        models::get_job(&mut conn, id)
    })
    .await??;
    log::info!(target: TARGET, "Request done");
    Ok(Json(job.into()))
}

#[put("/jobs/{id}")]
pub async fn rejudge_job(
    id: Path<i32>,
    config: Data<Config>,
    pool: Data<DbPool>,
) -> Result<Json<Job>, Error> {
    const TARGET: &str = "PUT /jobs/{id}";
    log::info!(target: TARGET, "Request received");

    let conn = &mut web::block(move || pool.get()).await??;

    let id = id.into_inner();
    let job_exists = models::does_job_exist(conn, id)?;
    if !job_exists {
        log::info!(target: TARGET, "No such job: {id}");
        Err(Error::new(
            Reason::NotFound,
            format!("Job {} not found.", id),
        ))
    } else {
        // Guard that the job is in Finished state
        let job = models::get_job(conn, id)?;
        if job.job_state != JobStatus::Finished {
            log::info!(
                target: TARGET,
                "Job {id} not finished: it's in {:?} state",
                job.job_state
            );
            return Err(Error::new(
                Reason::InvalidState,
                format!("Job {id} not finished."),
            ));
        }

        // Modify the state to be running
        let job: Job = models::update_job(
            conn,
            models::Job {
                updated_time: Utc::now().naive_utc(),
                job_state: JobStatus::Running,
                result: JobResult::Waiting,
                score: 0.0,
                cases: CaseResults(vec![]),
                ..job.into()
            },
        )?
        .into();

        log::info!(target: TARGET, "Judging started");
        let result = judge(
            &job.submission.source_code,
            config.get_lang(&job.submission.language).unwrap(),
            config.get_problem(job.submission.problem_id).unwrap(),
        );
        log::info!(target: TARGET, "Judging ended, result: {:?}", result.result);

        // Update the job
        let job = models::Job {
            updated_time: Utc::now().naive_utc(),
            job_state: JobStatus::Finished,
            result: result.result,
            score: result.score,
            cases: CaseResults(result.cases),
            ..job.into()
        };
        let job = models::update_job(conn, job)?;
        log::info!(target: TARGET, "Job {} updated", id);
        log::info!(target: TARGET, "Request done");
        Ok(Json(job.into()))
    }
}
