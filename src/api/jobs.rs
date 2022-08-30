use actix_web::{web::{Json, Data}, post};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

use super::err::{Reason, Error};

use crate::{config::Config, judge::judge};

#[derive(Serialize, Deserialize)]
pub struct Submission {
    pub source_code: String,
    pub language: String,
    pub user_id: u32,
    pub contest_id: u32,
    pub problem_id: u32,
}

#[derive(Serialize)]
pub enum JobStatus {
    Queueing,
    Running,
    Finished,
    Canceled,
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Serialize)]
pub struct CaseResult {
    pub id: u32,
    pub result: JobResult,
    pub time: u32,
    pub memory: u32,
}

#[derive(Serialize)]
pub struct Job {
    pub id: u32,
    pub created_time: DateTime<Utc>,
    pub updated_time: DateTime<Utc>,
    pub submission: Submission,
    pub state: JobStatus,
    pub result: JobResult,
    pub score: f64,
    pub cases: Vec<CaseResult>,
}

#[post("/jobs")]
pub async fn new_job(submission: Json<Submission>, config: Data<Config>) -> Result<Json<Job>, Error> {
    log::info!(target: "jobs", "New job received");
    match config.get_lang(&submission.language) {
        None => {
            Err(Error::new(Reason::NotFound, format!("No such language: {}", submission.language)))
        }
        Some(lang) => {
            match config.get_problem(submission.problem_id) {
                None => {
                    Err(Error::new(Reason::NotFound, format!("No such problem: {}", submission.problem_id)))
                }
                Some(problem) => {
                    log::info!(target: "jobs", "Begin judging");
                    let created = Utc::now();
                    let result = judge(&submission.source_code, lang, problem);
                    log::info!(target: "jobs", "Ended judging");
                    Ok(Json(Job {
                        id: 0,
                        created_time: created,
                        updated_time: Utc::now(),
                        submission: submission.into_inner(),
                        state: JobStatus::Finished,
                        result: result.0,
                        score: result.1,
                        cases: result.2,
                    }).into())
                }
            }
        }
    }
}