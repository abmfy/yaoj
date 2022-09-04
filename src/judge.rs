use std::fs::{self, File};
use std::io::{self, Read};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use temp_dir::TempDir;
use wait_timeout::ChildExt;

use crate::api::jobs::{CaseResult, Job, JobResult, JobStatus};
use crate::config::{Language, Problem, ProblemType};
use crate::persistent::models;
use crate::DbPool;

/// Auxiliary function for reading from a file
fn read(mut f: File) -> Result<String, io::Error> {
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Auxiliary function for trimming whitespace
fn trim(f: File) -> Result<String, io::Error> {
    let buf = read(f)?;

    // Trim whitespace at EOF
    let buf = buf.trim_end();

    // Trim whitespace at EOF
    let mut result = String::new();
    for line in buf.split('\n') {
        result.push_str(line.trim_end());
    }
    Ok(result)
}

/// Judge given code and update the result in real time
pub fn judge(pool: Arc<DbPool>, code: &str, lang: &Language, problem: &Problem, mut job: Job) {
    let target = &format!("judge@job{}", job.id);
    log::info!(
        target: target,
        "New judge task started, lang: {}, problem id: {}",
        lang.name,
        problem.id
    );

    let conn = &mut pool.get().unwrap();

    // Push update to database
    macro_rules! push {
        () => {
            job.updated_time = Utc::now();
            models::update_job(conn, job.clone().into()).unwrap();
        };
    }

    // Create a temp directory for use
    let dir = TempDir::new().unwrap();

    // Save code to a source file
    let source = dir.child(&lang.file_name);
    fs::write(source.clone(), code).unwrap();

    // Executable file
    let exec = dir.child("main");

    // Substitute %INPUT% and %OUTPUT% in args
    let args: Vec<&str> = lang
        .command
        .iter()
        .map(|arg| match arg.as_ref() {
            "%INPUT%" => source.to_str().unwrap(),
            "%OUTPUT%" => exec.to_str().unwrap(),
            _ => arg,
        })
        .collect();

    // Compile
    let now = Instant::now();
    job.state = JobStatus::Running;
    push!();

    let result = Command::new(args[0]).args(args.iter().skip(1)).status();

    // Compilation error
    if result.is_err() || !result.unwrap().success() {
        log::info!(target: target, "Compilation error");
        job = Job {
            state: JobStatus::Finished,
            result: JobResult::CompilationError,
            ..job
        };
        job.cases[0] = CaseResult {
            id: 0,
            result: JobResult::CompilationError,
            time: now.elapsed().as_micros() as u32,
            memory: 0,
        };
        push!();
        return;
    }

    // Compilation success
    job.cases[0] = CaseResult {
        id: 0,
        result: JobResult::CompilationSuccess,
        time: now.elapsed().as_micros() as u32,
        memory: 0,
    };
    push!();

    // Intermediate job result
    let mut job_result = JobResult::Accepted;

    // Judge
    for (id, case) in problem.cases.iter().enumerate() {
        let id = id as u32 + 1;
        let case_result = &mut job.cases[id as usize];

        // Auxiliary macro for reporting an system error
        macro_rules! system_error {
            ($($x:tt)+) => {
                log::error!(target: target, $($x)+);
                if job_result == JobResult::Accepted {
                    job_result = JobResult::SystemError;
                }
                case_result.result = JobResult::SystemError;
                push!();
                continue;
            };
        }

        let input = File::open(case.input_file.clone());
        let output = File::create(dir.child(".output"));

        // Unable to open file
        if input.is_err() {
            system_error!("Unable to open input file: {}", input.unwrap_err());
        }
        if output.is_err() {
            system_error!("Unable to open output file: {}", output.unwrap_err());
        }

        // Child process
        let child = Command::new(exec.clone())
            .stdin(input.unwrap())
            .stdout(output.unwrap())
            .spawn();

        // Unable to spawn process
        if child.is_err() {
            system_error!("Unable to spawn process: {}", child.unwrap_err());
        }

        let mut child = child.unwrap();

        let now = Instant::now();

        // Auxiliary macro for updating results
        macro_rules! update_result {
            ($result: expr, $($x:tt)+) => {
                log::info!(target: target, $($x)+);

                // Record first non-accepted result
                if $result != JobResult::Accepted && job_result == JobResult::Accepted {
                    job_result = $result;
                }
                case_result.result = $result;
                case_result.time = now.elapsed().as_micros() as u32;

                push!();
                continue;
            };
        }

        // Wait for the process to finish and check status code
        match child.wait_timeout(if case.time_limit != 0 {
            Duration::from_micros(case.time_limit as u64) + Duration::from_millis(500)
        } else {
            Duration::MAX
        }) {
            Ok(Some(status)) => {
                // Exited, but with an error
                if !status.success() {
                    update_result!(JobResult::RuntimeError, "Test case {id}: Runtime error");
                }
            }
            // Child hasn't exited yet
            Ok(None) => {
                match child.kill() {
                    Ok(_) => {
                        update_result!(
                            JobResult::TimeLimitExceeded,
                            "Test case {id}: Time limit exceeded"
                        );
                    }
                    Err(err) => {
                        system_error!("Unable to kill child process: {}", err);
                    }
                };
            }
            // Unknown error
            Err(err) => {
                let _ = child.kill();
                system_error!("Unknown error when executing program: {}", err);
            }
        };

        // Check if time limit exceeded
        if case.time_limit != 0 && now.elapsed().as_micros() as u32 > case.time_limit {
            update_result!(
                JobResult::TimeLimitExceeded,
                "Test case {id}: Time limit exceeded"
            );
        }

        // Open the output file again
        let output = File::open(dir.child(".output"));
        if output.is_err() {
            system_error!("Unable to open output file: {}", output.unwrap_err());
        }
        let output = output.unwrap();

        // Open the answer file
        let answer = File::open(case.answer_file.clone());
        if answer.is_err() {
            system_error!("Unable to open answer file: {}", answer.unwrap_err());
        }
        let answer = answer.unwrap();

        // Now we are sure that the process exited successfully
        // Check the output
        let (output, answer) = match problem.typ {
            ProblemType::Standard => (trim(output), trim(answer)),
            ProblemType::Strict => (read(output), read(answer)),
            _ => {
                system_error!("Unimplemented problem type");
            }
        };

        if output.is_err() {
            system_error!("Unable to read from output file: {}", output.unwrap_err());
        }
        if answer.is_err() {
            system_error!("Unable to read from answer file: {}", answer.unwrap_err());
        }

        let (output, answer) = (output.unwrap(), answer.unwrap());

        if output == answer {
            job.score += case.score;
            update_result!(JobResult::Accepted, "Test case {id}: Accepted");
        } else {
            log::info!(target: target, "Output: {output}*EOF*");
            log::info!(target: target, "Answer: {answer}*EOF*");
            update_result!(JobResult::WrongAnswer, "Test case {id}: Wrong Answer");
        }
    }

    job.state = JobStatus::Finished;
    job.result = job_result;
    push!();

    log::info!(target: target, "Judging ended");
}
