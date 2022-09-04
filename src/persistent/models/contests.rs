use chrono::NaiveDateTime;
use diesel::prelude::*;
use serde::Serialize;

use crate::api::err::{Error, Reason};
use crate::persistent::schema::contests;

#[derive(Clone, Serialize, Queryable, Insertable, AsChangeset, Identifiable)]
pub struct Contest {
    pub id: i32,
    pub contest_name: String,
    pub contest_from: NaiveDateTime,
    pub contest_to: NaiveDateTime,
    pub problem_ids: String,
    pub user_ids: String,
    pub submission_limit: i32,
}

impl From<crate::api::contests::Contest> for Contest {
    fn from(contest: crate::api::contests::Contest) -> Self {
        Self {
            id: contest.id.unwrap() as i32,
            contest_name: contest.name,
            contest_from: contest.from.naive_utc(),
            contest_to: contest.to.naive_utc(),
            problem_ids: contest
                .problem_ids
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(","),
            user_ids: contest
                .user_ids
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>()
                .join(","),
            submission_limit: contest.submission_limit as i32,
        }
    }
}

/// Whether a contest exists
pub fn does_contest_exist(conn: &mut SqliteConnection, cid: i32) -> Result<bool, Error> {
    use self::contests::dsl::*;

    Ok(contests
        .find(cid)
        .first::<Contest>(conn)
        .optional()?
        .is_some())
}

/// Get contests count
pub fn contests_count(conn: &mut SqliteConnection) -> Result<i32, Error> {
    use self::contests::dsl::*;

    let count: i64 = contests.count().get_result(conn)?;

    Ok(count as i32)
}

/// Get contest by id
pub fn get_contest(conn: &mut SqliteConnection, cid: i32) -> Result<Contest, Error> {
    use self::contests::dsl::*;

    contests
        .find(cid)
        .first(conn)
        .optional()?
        .ok_or_else(|| Error::new(Reason::NotFound, format!("Contest {cid} not found.")))
}

/// Get all contests
pub fn get_contests(conn: &mut SqliteConnection) -> Result<Vec<Contest>, Error> {
    use self::contests::dsl::*;

    Ok(contests.load(conn)?)
}

pub fn new_contest(conn: &mut SqliteConnection, con: Contest) -> Result<Contest, Error> {
    use self::contests::dsl::*;

    diesel::insert_into(contests)
        .values(con.clone())
        .execute(conn)?;
    Ok(con)
}

pub fn update_contest(conn: &mut SqliteConnection, con: Contest) -> Result<Contest, Error> {
    Ok(con.save_changes(conn)?)
}
