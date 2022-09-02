use std::sync::{Arc, Mutex};

use actix_web::{get, post, web::Json};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

use super::err::{Error, Reason};

lazy_static! {
    pub static ref USERS: Arc<Mutex<Vec<User>>> = Arc::new(Mutex::new(vec![User {
        id: 0,
        name: "root".to_string(),
    }]));
}

#[derive(Clone, Serialize)]
pub struct User {
    pub id: u32,
    pub name: String,
}

#[derive(Deserialize)]
pub struct UserUpdate {
    id: Option<u32>,
    name: String,
}

/// Get user id by username
pub fn get_id_by_username(username: &str) -> Option<u32> {
    let users = USERS.lock().unwrap();
    for user in users.iter() {
        if user.name == username {
            return Some(user.id);
        }
    }
    None
}

/// Returns whether the user exists
pub fn does_user_exist(id: u32) -> bool {
    let users = USERS.lock().unwrap();
    id < users.len() as u32
}

#[post("/users")]
pub async fn update_user(user: Json<UserUpdate>) -> Result<Json<User>, Error> {
    const TARGET: &str = "POST /users";
    log::info!(target: TARGET, "Request received");

    let UserUpdate { id, name } = user.into_inner();

    let mut users = USERS.lock().unwrap();

    // Check if the username is used
    if users.iter().any(|user| user.name == name) {
        log::info!(target: TARGET, "Username {name} is used");
        return Err(Error::new(
            Reason::InvalidArgument,
            format!("Username '{name}' already exists."),
        ));
    }

    // Update user
    if let Some(id) = id {
        if id >= users.len() as u32 {
            log::info!(target: TARGET, "No such user: {id}");
            return Err(Error::new(
                Reason::NotFound,
                format!("User {id} not found."),
            ));
        }
        users[id as usize].name = name;
        log::info!(target: TARGET, "Request done");
        Ok(Json(users[id as usize].clone()))
    } else {
        // Create user
        let id = users.len() as u32;
        let user = User { id, name };
        users.push(user.clone());
        log::info!(target: TARGET, "Request done");
        Ok(Json(user))
    }
}

#[get("/users")]
pub async fn get_users() -> Result<Json<Vec<User>>, Error> {
    const TARGET: &str = "GET /users";
    log::info!(target: TARGET, "Request received");

    let users = USERS.lock().unwrap();
    log::info!(target: TARGET, "Request done");
    Ok(Json(users.clone()))
}
