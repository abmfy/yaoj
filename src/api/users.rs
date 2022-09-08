use actix_web::{
    get, post,
    web::{self, Data, Json},
};

use crate::{persistent::models, DbPool};

#[cfg(feature = "authorization")]
use crate::{
    api::err::Reason,
    authorization::{Role, UserClaims},
};

use super::err::Error;

#[post("/users")]
pub async fn update_user(
    user: Json<models::UserForm>,
    pool: Data<DbPool>,
    #[cfg(feature = "authorization")] user_claims: UserClaims,
) -> Result<Json<models::User>, Error> {
    const TARGET: &str = "POST /users";
    log::info!(target: TARGET, "Request received");

    #[cfg(feature = "authorization")]
    if user_claims.role < Role::Admin {
        log::info!(target: TARGET, "Forbidden");
        return Err(Error::new(
            Reason::Forbidden,
            "You have no permission to access this service".to_string(),
        ));
    }

    let user = web::block(move || {
        let mut conn = pool.get()?;
        models::update_user(&mut conn, user.into_inner())
    })
    .await??;

    log::info!(target: TARGET, "Request done");
    Ok(Json(user))
}

#[get("/users")]
pub async fn get_users(pool: Data<DbPool>) -> Result<Json<Vec<models::User>>, Error> {
    const TARGET: &str = "GET /users";
    log::info!(target: TARGET, "Request received");

    let users = web::block(move || {
        let mut conn = pool.get()?;
        models::get_users(&mut conn)
    })
    .await??;

    log::info!(target: TARGET, "Request done");
    Ok(Json(users))
}
