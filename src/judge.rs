use std::fs::{self, File};
use std::io::{self, Read};
use std::process::Command;
use std::time::{Duration, Instant};

use temp_dir::TempDir;
use wait_timeout::ChildExt;

use crate::api::jobs::{CaseResult, JobResult};
use crate::config::{Language, Problem, ProblemType};

pub struct JudgeResult {
    pub result: JobResult,
    pub score: f64,
    pub cases: Vec<CaseResult>,
}

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

/// Judge given code and return the result
pub fn judge(code: &str, lang: &Language, problem: &Problem) -> JudgeResult {
    const TARGET: &str = "judge";
    log::info!(
        target: TARGET,
        "New judge task started, lang: {}, problem id: {}",
        lang.name,
        problem.id
    );

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
    let result = Command::new(args[0]).args(args.iter().skip(1)).status();

    // Compilation error
    if result.is_err() || !result.unwrap().success() {
        log::info!(target: TARGET, "Compilation error");
        let mut results = vec![CaseResult {
            id: 0,
            result: JobResult::CompilationError,
            time: now.elapsed().as_micros() as u32,
            memory: 0,
        }];
        for i in 1..=problem.cases.len() {
            results.push(CaseResult {
                id: i as u32,
                result: JobResult::Waiting,
                time: 0,
                memory: 0,
            });
        }
        return JudgeResult {
            result: JobResult::CompilationError,
            score: 0.0,
            cases: results,
        };
    }

    // Compilation success

    // Judge results
    let mut score = 0.0;
    let mut result_type: JobResult = JobResult::Accepted;
    let mut results = vec![CaseResult {
        id: 0,
        result: JobResult::CompilationSuccess,
        time: now.elapsed().as_micros() as u32,
        memory: 0,
    }];

    // Judge
    for case in problem.cases.iter() {
        let id = results.len() as u32;
        // Auxiliary function for reporting an system error
        fn system_error(results: &mut Vec<CaseResult>) {
            results.push(CaseResult {
                id: results.len() as u32,
                result: JobResult::SystemError,
                time: 0,
                memory: 0,
            });
        }

        let input = File::open(case.input_file.clone());
        let output = File::create(dir.child("output"));

        // Unable to open file
        if input.is_err() {
            log::error!(
                target: TARGET,
                "Unable to open input file: {}",
                input.unwrap_err()
            );
            system_error(&mut results);
            continue;
        }
        if output.is_err() {
            log::error!(
                target: TARGET,
                "Unable to open output file: {}",
                output.unwrap_err()
            );
            system_error(&mut results);
            continue;
        }

        // Child process
        let child = Command::new(exec.clone())
            .stdin(input.unwrap())
            .stdout(output.unwrap())
            .spawn();

        // Unable to spawn process
        if child.is_err() {
            log::error!(
                target: TARGET,
                "Unable to spawn process: {}",
                child.unwrap_err()
            );
            system_error(&mut results);
            continue;
        }

        let mut child = child.unwrap();

        let now = Instant::now();

        // Auxiliary function for inserting a new result
        let mut update_result = |result: JobResult, time: u32| {
            // Record first non-accepted result
            if result != JobResult::Accepted && result_type == JobResult::Accepted {
                result_type = result;
            }
            results.push(CaseResult {
                id,
                result,
                time,
                memory: 0,
            });
        };

        // Wait for the process to finish and check status code
        match child.wait_timeout(if case.time_limit != 0 {
            Duration::from_micros(case.time_limit as u64) + Duration::from_millis(500)
        } else {
            Duration::MAX
        }) {
            Ok(Some(status)) => {
                // Exited, but with an error
                if !status.success() {
                    log::info!(target: TARGET, "Test case {id}: Runtime error");
                    update_result(JobResult::RuntimeError, 0);
                    continue;
                }
            }
            // Child hasn't exited yet
            Ok(None) => {
                match child.kill() {
                    Ok(_) => {
                        log::info!(target: TARGET, "Test case {id}: Time limit exceeded");
                        update_result(
                            JobResult::TimeLimitExceeded,
                            now.elapsed().as_micros() as u32,
                        );
                    }
                    Err(err) => {
                        log::error!(target: TARGET, "Unable to kill child process: {}", err);
                        system_error(&mut results);
                    }
                };
                continue;
            }
            // Unknown error
            Err(err) => {
                let _ = child.kill();
                log::error!(
                    target: TARGET,
                    "Unknown error when executing program: {}",
                    err
                );
                system_error(&mut results);
                continue;
            }
        };

        let time = now.elapsed().as_micros() as u32;

        // Check if time limit exceeded
        if case.time_limit != 0 && time > case.time_limit as u32 {
            log::info!(target: TARGET, "Test case {id}: Time limit exceeded");
            update_result(JobResult::TimeLimitExceeded, time);
            continue;
        }

        // Open the output file again
        let output = File::open(dir.child("output"));
        if output.is_err() {
            log::error!(
                target: TARGET,
                "Unable to open output file: {}",
                output.unwrap_err()
            );
            system_error(&mut results);
            continue;
        }
        let output = output.unwrap();

        // Open the answer file
        let answer = File::open(case.answer_file.clone());
        if answer.is_err() {
            log::error!(
                target: TARGET,
                "Unable to open answer file: {}",
                answer.unwrap_err()
            );
            system_error(&mut results);
            continue;
        }
        let answer = answer.unwrap();

        // Now we are sure that the process exited successfully
        // Check the output
        match problem.typ {
            ProblemType::Standard => {
                let output = trim(output);
                if output.is_err() {
                    log::error!(
                        target: TARGET,
                        "Unable to read from output file: {}",
                        output.unwrap_err()
                    );
                    system_error(&mut results);
                    continue;
                }
                let answer = trim(answer);
                if output.is_err() {
                    log::error!(
                        target: TARGET,
                        "Unable to read from answer file: {}",
                        answer.unwrap_err()
                    );
                    system_error(&mut results);
                    continue;
                }
                let output = output.unwrap();
                let answer = answer.unwrap();

                if output == answer {
                    log::info!(target: TARGET, "Test case {id}: Accepted");
                    score += case.score;
                    update_result(JobResult::Accepted, time);
                } else {
                    log::info!(target: TARGET, "Test case {id}: Wrong Answer");
                    log::info!(target: TARGET, "Output: {output}*EOF*");
                    log::info!(target: TARGET, "Answer: {answer}*EOF*");
                    update_result(JobResult::WrongAnswer, time);
                }
            }
            ProblemType::Strict => {
                let output = read(output);
                if output.is_err() {
                    log::error!(
                        target: TARGET,
                        "Unable to read from output file: {}",
                        output.unwrap_err()
                    );
                    system_error(&mut results);
                    continue;
                }
                let answer = read(answer);
                if output.is_err() {
                    log::error!(
                        target: TARGET,
                        "Unable to read from answer file: {}",
                        answer.unwrap_err()
                    );
                    system_error(&mut results);
                    continue;
                }
                let output = output.unwrap();
                let answer = answer.unwrap();

                if output == answer {
                    log::info!(target: TARGET, "Test case {id}: Accepted");
                    score += case.score;
                    update_result(JobResult::Accepted, time);
                } else {
                    log::info!(target: TARGET, "Test case {id}: Wrong Answer");
                    log::info!(target: TARGET, "Output: {output}*EOF*");
                    log::info!(target: TARGET, "Answer: {answer}*EOF*");
                    update_result(JobResult::WrongAnswer, time);
                }
            }
            _ => unimplemented!(),
        }
    }

    JudgeResult {
        result: result_type,
        score,
        cases: results,
    }
}
