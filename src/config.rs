use std::{fs::File, path::PathBuf};

use clap::Parser;
use serde::Deserialize;
use serde_json::Value;

fn get_default_address() -> String {
    "127.0.0.1".into()
}

fn get_default_port() -> u16 {
    12345
}

/// Server config
#[derive(Clone, Deserialize)]
pub struct Server {
    #[serde(default = "get_default_address")]
    pub bind_address: String,
    #[serde(default = "get_default_port")]
    pub bind_port: u16,
}

/// A test case of a problem
#[derive(Clone, Deserialize)]
pub struct Case {
    pub score: f64,
    pub input_file: PathBuf,
    pub answer_file: PathBuf,
    pub time_limit: u32,
    pub memory_limit: u32,
}

/// Problem type
#[derive(Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemType {
    Standard,
    Strict,
    Spj,
    DynamicRanking,
}

/// A problem
#[derive(Clone, Deserialize)]
pub struct Problem {
    pub id: u32,
    pub name: String,
    #[serde(rename = "type")]
    pub typ: ProblemType,
    pub misc: Option<Value>,
    pub cases: Vec<Case>,
}

/// An available programming language
#[derive(Clone, Deserialize)]
pub struct Language {
    pub name: String,
    pub file_name: String,
    pub command: Vec<String>,
}

/// Startup configuration
#[derive(Clone, Deserialize)]
pub struct Config {
    pub server: Server,
    pub problems: Vec<Problem>,
    pub languages: Vec<Language>,
}

impl Config {
    /// Get the config for a specified language
    pub fn get_lang(&self, lang: &str) -> Option<&Language> {
        self.languages.iter().find(|l| l.name == lang)
    }

    /// Get specified problem
    pub fn get_problem(&self, id: u32) -> Option<&Problem> {
        self.problems.iter().find(|p| p.id == id)
    }
}

#[derive(Parser)]
#[clap(author = "abmfy", about = "Yet Another Online Judge")]
pub struct Args {
    /// Path of the configuration file in JSON format
    #[clap(short, long, value_parser = parse_config)]
    pub config: (String, Config),

    /// Whether to flush persistent data
    #[clap(short, long)]
    pub flush_data: bool,

    /// Run this process as judger process with given id
    #[clap(short, long)]
    pub judger: Option<i32>,

    /// The parent of this judger
    #[clap(short, long)]
    pub parent: Option<u32>
}

fn parse_config(path: &str) -> Result<(String, Config), std::io::Error> {
    let path_str = path.to_string();
    let path = PathBuf::from(path);
    let file = File::open(path)?;
    let config: Config = serde_json::from_reader(file)?;
    Ok((path_str, config))
}
