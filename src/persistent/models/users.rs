use diesel::prelude::*;

use serde::{Deserialize, Serialize};

use crate::api::err::{Error, Reason};
use crate::persistent::schema::users;

#[derive(Insertable, AsChangeset, Deserialize)]
#[diesel(table_name = users)]
pub struct UserForm {
    pub id: Option<i32>,
    #[serde(rename = "name")]
    pub user_name: String,
}

#[derive(Queryable, Serialize)]
pub struct User {
    pub id: i32,
    #[serde(rename = "name")]
    pub user_name: String,
}

/// Returns if the user with specified id exists
pub fn does_user_exist(conn: &mut SqliteConnection, uid: i32) -> Result<bool, Error> {
    use self::users::dsl::*;

    let user = users.find(uid).first::<User>(conn).optional()?;

    Ok(user.is_some())
}

/// Returns how many users are there
pub fn user_count(conn: &mut SqliteConnection) -> Result<i32, Error> {
    use self::users::dsl::*;

    let count: i64 = users.count().get_result(conn)?;

    Ok(count as i32)
}

/// Get user id by username
pub fn get_id_by_username(conn: &mut SqliteConnection, name: &str) -> Result<Option<i32>, Error> {
    use self::users::dsl::*;

    let uid: Option<i32> = users
        .select(id)
        .filter(user_name.eq(name))
        .first(conn)
        .optional()?;

    Ok(uid)
}

/// Update or insert a user
pub fn update_user(conn: &mut SqliteConnection, user_form: UserForm) -> Result<User, Error> {
    use self::users::dsl::*;

    let uid = user_form.id;
    let name = &user_form.user_name;

    // Check if the username is used
    let user: Option<User> = users.filter(user_name.eq(name)).first(conn).optional()?;

    if let Some(user) = user {
        if uid.is_none() || uid.unwrap() != user.id {
            return Err(Error::new(
                Reason::InvalidArgument,
                format!("Username '{}' already exists.", name),
            ));
        }
    }

    // Update mode
    if let Some(uid) = uid {
        let user: Option<User> = users.find(uid).first(conn).optional()?;
        if user.is_some() {
            Ok(diesel::update(users.find(uid))
                .set(user_name.eq(name))
                .get_result(conn)?)
        } else {
            Err(Error::new(
                Reason::NotFound,
                format!("User {uid} not found."),
            ))
        }
    } else {
        // Insert mode
        Ok(diesel::insert_into(users)
            .values(user_form)
            .get_result(conn)?)
    }
}

/// Get user by id
pub fn get_user(conn: &mut SqliteConnection, uid: i32) -> Result<User, Error> {
    use self::users::dsl::*;

    users
        .find(uid)
        .first(conn)
        .optional()?
        .ok_or_else(|| Error::new(Reason::NotFound, format!("User {uid} not found.")))
}

/// Get selected users
pub fn get_some_users(conn: &mut SqliteConnection, ids: Vec<i32>) -> Result<Vec<User>, Error> {
    use self::users::dsl::*;

    Ok(users.filter(id.eq_any(ids)).load(conn)?)
}

/// Get all users
pub fn get_users(conn: &mut SqliteConnection) -> Result<Vec<User>, Error> {
    use self::users::dsl::*;

    Ok(users.load(conn)?)
}
