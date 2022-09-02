use std::{cmp::Ordering, collections::HashMap};

use actix_web::{
    get,
    web::{Data, Json, Query},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{api::users::USERS, config::Config};

use super::{err::Error, jobs::JOBS, users::User};

#[derive(Serialize, Deserialize)]
struct Contest {
    id: u32,
    name: String,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    problem_ids: Vec<u32>,
    use_ids: Vec<u32>,
    submission_limit: u32,
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
            TieBreaker::UserId => id_a.cmp(&id_b),
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

#[get("/contests/0/ranklist")]
pub async fn get_rank_list(
    rule: Query<RankingRule>,
    config: Data<Config>,
) -> Result<Json<Vec<RankingItem>>, Error> {
    const TARGET: &str = "GET /contests/0/ranklist";
    log::info!(target: TARGET, "Request received");

    let RankingRule {
        scoring_rule,
        tie_breaker,
    } = rule.into_inner();

    let scoring_rule = scoring_rule.unwrap_or(ScoringRule::Latest);
    let tie_breaker = tie_breaker.unwrap_or(TieBreaker::Default);

    // submissions[id] means the submissions for user 'id', and
    // it is a hash map 'm' where m[p_id] means the user's score on problem 'p_id'
    let users = USERS.lock().unwrap();
    let user_count = users.len();
    drop(users);
    let mut submissions: Vec<HashMap<u32, ProblemResult>> = vec![HashMap::new(); user_count];

    let jobs = JOBS.lock().unwrap();
    for job in jobs.iter() {
        let user_id = job.submission.user_id;
        let problem_id = job.submission.problem_id;
        let score = job.score;
        let submission_time = job.created_time;

        // Fetch the problem results for a user
        let problem_results = &mut submissions[user_id as usize];

        // Set the problem results for each problem according to the scoring rule
        problem_results
            .entry(problem_id)
            .and_modify(|result| {
                result.submission_count += 1;
                match scoring_rule {
                    ScoringRule::Latest => {
                        if submission_time > result.submission_time {
                            result.score = score;
                            result.submission_time = submission_time;
                        }
                    }
                    ScoringRule::Highest => {
                        if score > result.score {
                            result.score = score;
                            result.submission_time = submission_time;
                        }
                    }
                }
            })
            .or_insert(ProblemResult {
                score,
                submission_time,
                submission_count: 1,
            });
    }
    drop(jobs);

    // Ranking according to the tie breaker rule
    let mut rank_list: Vec<_> = submissions
        .iter()
        .enumerate()
        .map(|(k, v)| (k as u32, v))
        .collect();
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
    let users = USERS.lock().unwrap();
    for (rank, (user_id, _)) in rank_list.iter().enumerate() {
        let last_rank = response.last().map(|item| item.rank).unwrap_or_default();
        response.push(RankingItem {
            user: users[*user_id as usize].clone(),
            // Calculate rank
            rank: if rank == 0 {
                1
            } else {
                // If the two users are ranked equal by the tie breaker rule,
                // assign them the same ranking
                if tie_breaker.compare(&rank_list[rank], &rank_list[rank - 1]) == Ordering::Equal {
                    last_rank
                } else {
                    rank as u32 + 1
                }
            },
            scores: config
                .problems
                .iter()
                .map(|p| {
                    // If no submissions on a problem are found, set the score to 0
                    submissions[*user_id as usize]
                        .get(&p.id)
                        .map(|result| result.score)
                        .unwrap_or_default()
                })
                .collect(),
        })
    }

    log::info!(target: TARGET, "Request done");
    Ok(Json(response))
}
