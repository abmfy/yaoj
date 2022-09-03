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
pub fn does_user_exist(conn: &mut SqliteConnection, id: i32) -> Result<bool, Error> {
    use self::users::dsl::*;

    let user = users.find(id).first::<User>(conn).optional()?;

    Ok(user.is_some())
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

/// Get all users
pub fn get_users(conn: &mut SqliteConnection) -> Result<Vec<User>, Error> {
    use self::users::dsl::*;

    Ok(users.load(conn)?)
}
