use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use actix_web::{
    get, post,
    web::{self, Data, Json, Path, Query},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{api::err::Reason, config::Problem, persistent::models::User};
use crate::{config::Config, persistent::models, DbPool};

use super::err::Error;

#[derive(Serialize, Deserialize)]
pub struct Contest {
    pub id: Option<u32>,
    pub name: String,
    #[serde(serialize_with = "super::serialize_date_time")]
    pub from: DateTime<Utc>,
    #[serde(serialize_with = "super::serialize_date_time")]
    pub to: DateTime<Utc>,
    pub problem_ids: Vec<u32>,
    pub user_ids: Vec<u32>,
    pub submission_limit: u32,
}

impl From<models::Contest> for Contest {
    fn from(contest: models::Contest) -> Self {
        Self {
            id: Some(contest.id as u32),
            name: contest.contest_name,
            from: contest.contest_from.and_local_timezone(Utc).unwrap(),
            to: contest.contest_to.and_local_timezone(Utc).unwrap(),
            problem_ids: contest
                .problem_ids
                .split(',')
                .map(|s| s.parse::<u32>().unwrap())
                .collect(),
            user_ids: contest
                .user_ids
                .split(',')
                .map(|s| s.parse::<u32>().unwrap())
                .collect(),
            submission_limit: contest.submission_limit as u32,
        }
    }
}

#[post("/contests")]
pub async fn update_contest(
    contest: Json<Contest>,
    config: Data<Config>,
    pool: Data<DbPool>,
) -> Result<Json<Contest>, Error> {
    const TARGET: &str = "POST /contests";
    log::info!(target: TARGET, "Request received");

    let contest = contest.into_inner();

    let conn = &mut web::block(move || pool.get()).await??;

    // Check validity of problems
    let problem_set: HashSet<_> = config.problems.iter().map(|p| p.id).collect();
    for pid in &contest.problem_ids {
        if !problem_set.contains(pid) {
            log::info!(target: TARGET, "No such problem: {pid}");
            return Err(Error::new(
                Reason::NotFound,
                format!("Unknown problem: {pid}"),
            ));
        }
    }

    // Check validity of users
    let user_count = models::user_count(conn)? as u32;
    for uid in &contest.user_ids {
        if uid >= &user_count {
            log::info!(target: TARGET, "No such user: {uid}");
            return Err(Error::new(Reason::NotFound, format!("Unknown user: {uid}")));
        }
    }

    // Update
    if let Some(id) = contest.id {
        let contest =
            models::update_contest(conn, contest.into()).map_err(|err| match err.reason {
                // Give a more detailed description when not found
                Reason::NotFound => {
                    log::info!(target: TARGET, "No such contest: {id}");
                    Error::new(Reason::NotFound, format!("Contest {id} not found."))
                }
                _ => err,
            })?;
        log::info!(target: TARGET, "Request done");
        Ok(Json(contest.into()))
    } else {
        // Insert
        let cid = models::contests_count(conn)? as u32 + 1;
        let contest = Contest {
            id: Some(cid),
            ..contest
        };
        let contest = models::new_contest(conn, contest.into())?;
        log::info!(target: TARGET, "Request done");
        Ok(Json(contest.into()))
    }
}

#[get("/contests")]
pub async fn get_contests(pool: Data<DbPool>) -> Result<Json<Vec<Contest>>, Error> {
    const TARGET: &str = "GET /contests";
    log::info!(target: TARGET, "Request received");

    let conn = &mut web::block(move || pool.get()).await??;

    let contests: Vec<Contest> = models::get_contests(conn)?
        .into_iter()
        .map(|c| c.into())
        .collect();
    log::info!(target: TARGET, "Request done");
    Ok(Json(contests))
}

#[get("/contests/{id}")]
pub async fn get_contest(id: Path<u32>, pool: Data<DbPool>) -> Result<Json<Contest>, Error> {
    const TARGET: &str = "GET /contests/{id}";
    log::info!(target: TARGET, "Request received");

    let id = id.into_inner() as i32;

    let conn = &mut web::block(move || pool.get()).await??;

    let contest: Contest = models::get_contest(conn, id)
        .map_err(|err| match err.reason {
            Reason::NotFound => {
                log::info!(target: TARGET, "No such contest: {id}");
                Error::new(Reason::NotFound, format!("Contest {id} not found."))
            }
            _ => err,
        })?
        .into();
    log::info!(target: TARGET, "Request done");
    Ok(Json(contest))
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoringRule {
    Latest,
    Highest,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TieBreaker {
    Default,
    SubmissionTime,
    SubmissionCount,
    UserId,
}

impl TieBreaker {
    /// Compare the ranking of two users
    pub fn compare(
        &self,
        (id_a, a): &(u32, &HashMap<u32, ProblemResult>),
        (id_b, b): &(u32, &HashMap<u32, ProblemResult>),
    ) -> Ordering {
        let total_score_a: f64 = a.values().map(|result| result.score).sum();
        let total_score_b: f64 = b.values().map(|result| result.score).sum();
        match total_score_a.total_cmp(&total_score_b).reverse() {
            Ordering::Equal => (),
            ord => return ord,
        }
        // Now the total score is equal
        // This is where tie breaker come into effect
        match self {
            // In the default mode (tie breaker not explicitly specified), the users with
            // the same score will rank as the same
            TieBreaker::Default => Ordering::Equal,
            TieBreaker::SubmissionTime => {
                // If the user never submitted they'll be ranked last
                let a = a
                    .values()
                    .map(|x| x.submission_time)
                    .max()
                    .unwrap_or(DateTime::<Utc>::MAX_UTC);
                let b = b
                    .values()
                    .map(|x| x.submission_time)
                    .max()
                    .unwrap_or(DateTime::<Utc>::MAX_UTC);
                a.cmp(&b)
            }
            TieBreaker::SubmissionCount => {
                let a: u32 = a.values().map(|x| x.submission_count).sum();
                let b: u32 = b.values().map(|x| x.submission_count).sum();
                a.cmp(&b)
            }
            TieBreaker::UserId => id_a.cmp(id_b),
        }
    }
}

#[derive(Deserialize)]
pub struct RankingRule {
    pub scoring_rule: Option<ScoringRule>,
    pub tie_breaker: Option<TieBreaker>,
}

#[derive(Serialize)]
pub struct RankingItem {
    user: User,
    rank: u32,
    scores: Vec<f64>,
}

// The result of a problem for a user, for ranking
#[derive(Clone)]
pub struct ProblemResult {
    score: f64,
    submission_time: DateTime<Utc>,
    submission_count: u32,
}

#[get("/contests/{id}/ranklist")]
pub async fn get_rank_list(
    id: Path<u32>,
    rule: Query<RankingRule>,
    config: Data<Config>,
    pool: Data<DbPool>,
) -> Result<Json<Vec<RankingItem>>, Error> {
    const TARGET: &str = "GET /contests/{id}/ranklist";
    log::info!(target: TARGET, "Request received");

    let conn = &mut web::block(move || pool.get()).await??;

    let id = id.into_inner();

    if id != 0 && !models::does_contest_exist(conn, id as i32)? {
        log::info!(target: TARGET, "No such contest: {id}");
        return Err(Error::new(
            Reason::NotFound,
            format!("Contest {id} not found."),
        ));
    }

    let RankingRule {
        scoring_rule,
        tie_breaker,
    } = rule.into_inner();

    let scoring_rule = scoring_rule.unwrap_or(ScoringRule::Latest);
    let tie_breaker = tie_breaker.unwrap_or(TieBreaker::Default);

    let users: Vec<User>;
    let problems: Vec<&Problem>;

    if id == 0 {
        users = models::get_users(conn)?;
        problems = config.problems.iter().collect();
    } else {
        let contest: Contest = models::get_contest(conn, id as i32)?.into();
        users =
            models::get_some_users(conn, contest.user_ids.iter().map(|id| *id as i32).collect())?;
        problems = contest
            .problem_ids
            .iter()
            .filter_map(|id| config.get_problem(*id))
            .collect();
    }

    let mut rank_list: Vec<(u32, HashMap<u32, ProblemResult>)> = vec![];
    for user in &users {
        let mut map = HashMap::<u32, ProblemResult>::new();
        for problem in &problems {
            // Fetch the problem result for a user
            let result = match scoring_rule {
                ScoringRule::Latest => {
                    models::get_latest_submission(conn, user.id, problem.id as i32, id as i32)
                }
                ScoringRule::Highest => {
                    models::get_highest_submission(conn, user.id, problem.id as i32, id as i32)
                }
            }?;
            // No submission on this problem
            if result.is_none() {
                continue;
            }
            let job = result.unwrap();
            let score = job.score;
            let submission_time = job.created_time.and_local_timezone(Utc).unwrap();
            let count =
                models::get_submission_count(conn, user.id, problem.id as i32, id as i32)? as u32;
            map.insert(
                problem.id,
                ProblemResult {
                    score,
                    submission_time,
                    submission_count: count,
                },
            );
        }
        rank_list.push((user.id as u32, map));
    }

    // Ranking according to the tie breaker rule
    rank_list.sort_by(|(id_a, a), (id_b, b)| {
        match tie_breaker.compare(&(*id_a, a), &(*id_b, b)) {
            // If equal, sort in ascending order by user id
            // Note that this will not affect the ranking, which is decided by the tie breaker
            Ordering::Equal => id_a.cmp(id_b),
            ord => ord,
        }
    });

    // Construct the response
    let mut response: Vec<RankingItem> = vec![];
    for (rank, (user_id, _)) in rank_list.iter().enumerate() {
        let last_rank = response.last().map(|item| item.rank).unwrap_or_default();
        response.push(RankingItem {
            user: models::get_user(conn, *user_id as i32)?,
            // Calculate rank
            rank: if rank == 0 {
                1
            } else {
                // If the two users are ranked equal by the tie breaker rule,
                // assign them the same ranking
                if tie_breaker.compare(
                    &(rank_list[rank].0, &rank_list[rank].1),
                    &(rank_list[rank - 1].0, &rank_list[rank - 1].1),
                ) == Ordering::Equal
                {
                    last_rank
                } else {
                    rank as u32 + 1
                }
            },
            scores: problems
                .iter()
                .map(|p| {
                    // If no submissions on a problem are found, set the score to 0
                    match scoring_rule {
                        ScoringRule::Latest => models::get_latest_submission(
                            conn,
                            *user_id as i32,
                            p.id as i32,
                            id as i32,
                        ),
                        ScoringRule::Highest => models::get_highest_submission(
                            conn,
                            *user_id as i32,
                            p.id as i32,
                            id as i32,
                        ),
                    }
                    .unwrap()
                    .map(|s| s.score)
                    .unwrap_or_default()
                })
                .collect(),
        })
    }

    log::info!(target: TARGET, "Request done");
    Ok(Json(response))
}
