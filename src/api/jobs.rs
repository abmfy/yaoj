use std::sync::{Arc, Mutex};

use actix_web::{
    get, post, put,
    web::{Data, Json, Path, Query},
};
use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use super::err::{Error, Reason};
use super::users;

use crate::{config::Config, judge::judge};

lazy_static! {
    static ref JOBS: Arc<Mutex<Vec<Job>>> = Arc::new(Mutex::new(vec![]));
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Submission {
    pub source_code: String,
    pub language: String,
    pub user_id: u32,
    pub contest_id: u32,
    pub problem_id: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queueing,
    Running,
    Finished,
    Canceled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Clone, Serialize)]
pub struct CaseResult {
    pub id: u32,
    pub result: JobResult,
    pub time: u32,
    pub memory: u32,
}

#[derive(Clone, Serialize)]
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
/// Create a new submission
pub async fn new_job(
    submission: Json<Submission>,
    config: Data<Config>,
) -> Result<Json<Job>, Error> {
    const TARGET: &str = "POST /jobs";
    log::info!(target: TARGET, "Request received");
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
                    if !users::does_user_exist(submission.user_id) {
                        log::info!(target: TARGET, "No such user: {}", submission.user_id);
                        return Err(Error::new(
                            Reason::NotFound,
                            format!("No such user: {}", submission.user_id),
                        ));
                    }

                    let created = Utc::now();

                    // Add the job to the jobs list with Running status
                    let mut jobs = JOBS.lock().unwrap();
                    let id = jobs.len() as u32;
                    let job = Job {
                        id,
                        created_time: created,
                        updated_time: created,
                        submission: submission.clone(),
                        state: JobStatus::Running,
                        result: JobResult::Waiting,
                        score: 0.0,
                        cases: vec![],
                    };
                    log::info!(target: TARGET, "Job {} created", job.id);
                    jobs.push(job);
                    drop(jobs);

                    log::info!(target: TARGET, "Judging started");
                    let result = judge(&submission.source_code, lang, problem);
                    log::info!(target: TARGET, "Judging ended, result: {:?}", result.result);
                    // Add the job to the jobs list
                    let mut jobs = JOBS.lock().unwrap();
                    let job = Job {
                        updated_time: Utc::now(),
                        state: JobStatus::Finished,
                        result: result.result,
                        score: result.score,
                        cases: result.cases,
                        ..jobs[id as usize].clone()
                    };
                    jobs[id as usize] = job.clone();
                    log::info!(target: TARGET, "Job {} added", id);
                    log::info!(target: TARGET, "Request done");
                    Ok(Json(job))
                }
            }
        }
    }
}

#[derive(Deserialize)]
pub struct JobFilter {
    user_id: Option<u32>,
    user_name: Option<String>,
    contest_id: Option<u32>,
    problem_id: Option<u32>,
    language: Option<String>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    state: Option<JobStatus>,
    result: Option<JobResult>,
}

#[get("/jobs")]
pub async fn get_jobs(
    filter: Query<JobFilter>,
    config: Data<Config>,
) -> Result<Json<Vec<Job>>, Error> {
    const TARGET: &str = "GET /jobs";
    log::info!(target: TARGET, "Request received");

    let jobs = JOBS.lock().unwrap();
    let mut filtered_jobs = Vec::new();
    for job in jobs.iter() {
        if let Some(user_id) = filter.user_id {
            if job.submission.user_id != user_id {
                continue;
            }
        }
        if let Some(name) = &filter.user_name {
            if let Some(id) = users::get_id_by_username(name) {
                if job.submission.user_id != id {
                    continue;
                }
            } else {
                continue;
            }
        }
        // Unimplemented: contest_id
        if let Some(problem_id) = filter.problem_id {
            if job.submission.problem_id != problem_id {
                continue;
            }
        }
        if let Some(language) = &filter.language {
            if &job.submission.language != language {
                continue;
            }
        }
        if let Some(from) = filter.from {
            if job.created_time < from {
                continue;
            }
        }
        if let Some(to) = filter.to {
            if job.created_time > to {
                continue;
            }
        }
        if let Some(state) = filter.state {
            if job.state != state {
                continue;
            }
        }
        if let Some(result) = filter.result {
            if job.result != result {
                continue;
            }
        }
        filtered_jobs.push(job.clone());
    }
    log::info!(target: TARGET, "Request done");
    Ok(Json(filtered_jobs))
}

#[get("/jobs/{id}")]
pub async fn get_job(id: Path<u32>) -> Result<Json<Job>, Error> {
    const TARGET: &str = "GET /jobs/{id}";
    log::info!(target: TARGET, "Request received");

    let id = id.into_inner();
    let jobs = JOBS.lock().unwrap();
    if id >= jobs.len() as u32 {
        log::info!(target: TARGET, "No such job: {id}");
        Err(Error::new(Reason::NotFound, format!("Job {id} not found.")))
    } else {
        log::info!(target: TARGET, "Request done");
        Ok(Json(jobs[id as usize].clone()))
    }
}

#[put("/jobs/{id}")]
pub async fn rejudge_job(id: Path<u32>, config: Data<Config>) -> Result<Json<Job>, Error> {
    const TARGET: &str = "PUT /jobs/{id}";
    log::info!(target: TARGET, "Request received");

    let id = id.into_inner();
    let mut jobs = JOBS.lock().unwrap();
    if id >= jobs.len() as u32 {
        log::info!(target: TARGET, "No such job: {id}");
        Err(Error::new(
            Reason::NotFound,
            format!("Job {} not found.", id),
        ))
    } else {
        // Guard that the job is in Finished state
        if jobs[id as usize].state != JobStatus::Finished {
            log::info!(
                target: TARGET,
                "Job {id} not finished: it's in {:?} state",
                jobs[id as usize].state
            );
            return Err(Error::new(
                Reason::InvalidState,
                format!("Job {id} not finished."),
            ));
        }
        jobs[id as usize].state = JobStatus::Running;
        let job = jobs[id as usize].clone();
        drop(jobs);

        log::info!(target: TARGET, "Judging started");
        let result = judge(
            &job.submission.source_code,
            config.get_lang(&job.submission.language).unwrap(),
            config.get_problem(job.submission.problem_id).unwrap(),
        );
        log::info!(target: TARGET, "Judging ended, result: {:?}", result.result);

        let mut jobs = JOBS.lock().unwrap();
        jobs[id as usize] = Job {
            updated_time: Utc::now(),
            state: JobStatus::Finished,
            result: result.result,
            score: result.score,
            cases: result.cases,
            ..job
        };
        log::info!(target: TARGET, "Job {} updated", id);
        log::info!(target: TARGET, "Request done");
        Ok(Json(jobs[id as usize].clone()))
    }
}
