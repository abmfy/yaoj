use std::fs::{self, File};
use std::io::{self, Read};
use std::process::Command;
use std::time::{Duration, Instant};

use temp_dir::TempDir;
use wait_timeout::ChildExt;

use crate::api::{
    jobs::{CaseResult, JobResult},
};
use crate::config::{Language, Problem, ProblemType};

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
pub fn judge(code: &str, lang: &Language, problem: &Problem) -> (JobResult, f64, Vec<CaseResult>) {
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
        return (JobResult::CompilationError, 0.0, results);
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
            log::error!(target: "judge", "Unable to open input file: {}", input.unwrap_err());
            system_error(&mut results);
            continue;
        }
        if output.is_err() {
            log::error!(target: "judge", "Unable to open output file: {}", output.unwrap_err());
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
            log::error!(target: "judge", "Unable to spawn process: {}", child.unwrap_err());
            system_error(&mut results);
            continue;
        }

        let mut child = child.unwrap();

        let now = Instant::now();

        // Auxiliary function for inserting a new result
        let mut update_result = |result: JobResult| {
            // Record first non-accepted result
            if result != JobResult::Accepted && result_type == JobResult::Accepted {
                result_type = result;
            }
            results.push(CaseResult {
                id: results.len() as u32,
                result,
                time: now.elapsed().as_micros() as u32,
                memory: 0,
            });
        };

        // Wait for the process to finish and check status code
        match child.wait_timeout(Duration::from_micros(case.time_limit as u64) + Duration::from_millis(500)) {
            Ok(Some(status)) => {
                // Exited, but with an error
                if !status.success() {
                    update_result(JobResult::RuntimeError);
                    continue;
                }
            },
            // Child hasn't exited yet
            Ok(None) => {
                match child.kill() {
                    Ok(_) => {
                        update_result(JobResult::TimeLimitExceeded);
                    }
                    Err(err) => {
                        log::error!(target: "judge", "Unable to kill child process: {}", err);
                        system_error(&mut results);
                    }
                };
                continue;
            }
            // Unknown error
            Err(err) => {
                let _ = child.kill();
                log::error!(target: "judge", "Unknown error when executing program: {}", err);
                system_error(&mut results);
                continue;
            }
        };

        // Check if time limit exceeded
        if now.elapsed().as_micros() as u32 > case.time_limit {
            update_result(JobResult::TimeLimitExceeded);
            continue;
        }

        // Open the output file again
        let output = File::open(dir.child("output"));
        if output.is_err() {
            log::error!(target: "judge", "Unable to open output file: {}", output.unwrap_err());
            system_error(&mut results);
            continue;
        }
        let output = output.unwrap();

        // Open the answer file
        let answer = File::open(case.answer_file.clone());
        if answer.is_err() {
            log::error!(target: "judge", "Unable to open answer file: {}", answer.unwrap_err());
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
                    log::error!(target: "judge", "Unable to read from output file: {}", output.unwrap_err());
                    system_error(&mut results);
                    continue;
                }
                let answer = trim(answer);
                if output.is_err() {
                    log::error!(target: "judge", "Unable to read from answer file: {}", answer.unwrap_err());
                    system_error(&mut results);
                    continue;
                }
                let output = output.unwrap();
                let answer = answer.unwrap();
                log::info!(target: "judge", "Output: {output}*EOF*");
                log::info!(target: "judge", "Answer: {answer}*EOF*");

                if output == answer {
                    score += case.score;
                    update_result(JobResult::Accepted);
                } else {
                    update_result(JobResult::WrongAnswer);
                }
            }
            ProblemType::Strict => {
                let output = read(output);
                if output.is_err() {
                    log::error!(target: "judge", "Unable to read from output file: {}", output.unwrap_err());
                    system_error(&mut results);
                    continue;
                }
                let answer = read(answer);
                if output.is_err() {
                    log::error!(target: "judge", "Unable to read from answer file: {}", answer.unwrap_err());
                    system_error(&mut results);
                    continue;
                }
                let output = output.unwrap();
                let answer = answer.unwrap();

                if output == answer {
                    score += case.score;
                    update_result(JobResult::Accepted);
                } else {
                    update_result(JobResult::WrongAnswer);
                }
            }
            _ => unimplemented!()
        }
    }

    (result_type, score, results)
}
