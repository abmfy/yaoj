#[cfg(feature = "authorization")]
use actix_jwt_auth_middleware::{Authority, FromRequest};
#[cfg(feature = "authorization")]
use actix_web::{
    get, post,
    web::{Data, Json},
    HttpResponse,
};
use diesel::{
    backend::{self, Backend},
    deserialize::FromSql,
    serialize::{IsNull, Output, ToSql},
    sql_types::Integer,
    sqlite::Sqlite,
    AsExpression, FromSqlRow,
};
use serde::{Deserialize, Serialize};

#[cfg(feature = "authorization")]
use diesel::prelude::*;

#[cfg(feature = "authorization")]
use crate::{
    api::err::{Error, Reason},
    persistent::{
        models::{self, User},
        schema::users,
    },
    DbPool,
};

#[cfg(feature = "authorization")]
#[derive(Deserialize, Insertable)]
#[diesel(table_name = users)]
pub struct UserForm {
    id: Option<i32>,
    #[serde(default = "Role::default")]
    user_role: Role,
    #[serde(rename = "username")]
    user_name: String,
    #[serde(rename = "password")]
    passwd: String,
}

#[derive(
    Serialize,
    Deserialize,
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    AsExpression,
    FromSqlRow,
)]
#[diesel(sql_type = Integer)]
pub enum Role {
    User,
    Author,
    Admin,
}

impl Default for Role {
    fn default() -> Self {
        Role::User
    }
}

impl ToSql<Integer, Sqlite> for Role
where
    i32: ToSql<Integer, Sqlite>,
{
    fn to_sql<'a>(&'a self, out: &mut Output<'a, '_, Sqlite>) -> diesel::serialize::Result {
        out.set_value(*self as i32);
        Ok(IsNull::No)
    }
}

impl<DB> FromSql<Integer, DB> for Role
where
    DB: Backend,
    i32: FromSql<Integer, DB>,
{
    fn from_sql(bytes: backend::RawValue<DB>) -> diesel::deserialize::Result<Self> {
        match i32::from_sql(bytes)? {
            0 => Ok(Role::User),
            1 => Ok(Role::Author),
            2 => Ok(Role::Admin),
            x => Err(format!("Unrecognized enum variant {x}").into()),
        }
    }
}

#[cfg(feature = "authorization")]
#[derive(Serialize, Deserialize, Clone)]
pub struct UserClaims {
    pub id: u32,
    pub role: Role,
}

#[cfg(feature = "authorization")]
#[derive(Serialize, Deserialize, Clone, FromRequest)]
pub struct UserClaims {
    pub id: u32,
    pub role: Role,
}

/// Verify if a user has access to a certain API
#[cfg(feature = "authorization")]
fn verify_service_request(user_claims: UserClaims, required: Role) -> bool {
    user_claims.role >= required
}

#[cfg(feature = "authorization")]
pub async fn verify_service_request_user(user_claims: UserClaims) -> bool {
    verify_service_request(user_claims, Role::User)
}

/// Register a new user
#[cfg(feature = "authorization")]
#[post("/register")]
pub async fn register(user: Json<UserForm>, pool: Data<DbPool>) -> Result<Json<User>, Error> {
    const TARGET: &str = "POST /register";
    log::info!(target: TARGET, "Request received");

    let conn = &mut pool.get()?;

    let user = user.into_inner();

    if models::get_id_by_username(conn, &user.user_name)?.is_some() {
        log::info!(target: TARGET, "Username conflict: {}", user.user_name);
        return Err(Error::new(
            Reason::InvalidArgument,
            "User name already exists".to_string(),
        ));
    }

    use self::users::dsl::*;

    diesel::insert_into(users).values(user).execute(conn)?;

    log::info!(target: TARGET, "Request done");
    Ok(Json(users.order(id.desc()).first(conn)?))
}

/// Login
#[cfg(feature = "authorization")]
#[post("/login")]
pub async fn login(
    user: Json<UserForm>,
    pool: Data<DbPool>,
    auth_authority: Data<Authority<UserClaims>>,
) -> Result<HttpResponse, Error> {
    const TARGET: &str = "POST /login";
    log::info!(target: TARGET, "Request received");

    let conn = &mut pool.get()?;

    let user_form = user.into_inner();

    use self::users::dsl::*;

    let user = users
        .filter(user_name.eq(user_form.user_name))
        .first::<User>(conn)?;

    if user_form.passwd != user.passwd {
        log::info!(target: TARGET, "Wrong password");
        return Err(Error::new(
            Reason::InvalidArgument,
            "Wrong password".to_string(),
        ));
    }

    let mut cookie = auth_authority.create_signed_cookie(UserClaims {
        id: user.id as u32,
        role: user.user_role,
    })?;
    cookie.set_secure(false);

    log::info!(target: TARGET, "Request done");
    Ok(HttpResponse::Ok().cookie(cookie).json(user))
}

/// Change current user's password
#[cfg(feature = "authorization")]
#[post("/passwd")]
pub async fn change_password(
    new_passwd: Json<String>,
    pool: Data<DbPool>,
    user_claims: UserClaims,
) -> Result<HttpResponse, Error> {
    const TARGET: &str = "POST /passwd";
    log::info!(target: TARGET, "Request received");

    let conn = &mut pool.get()?;

    use self::users::dsl::*;

    diesel::update(users.find(user_claims.id as i32))
        .set(passwd.eq(new_passwd.into_inner()))
        .execute(conn)?;

    log::info!(target: TARGET, "Request done");
    Ok(HttpResponse::Ok().finish())
}

#[cfg(feature = "authorization")]
#[derive(Deserialize)]
pub struct PrivilegeForm {
    username: String,
    role: Role,
}

/// Change user's role
#[cfg(feature = "authorization")]
#[post("/privilege")]
pub async fn privilege(
    privilege: Json<PrivilegeForm>,
    pool: Data<DbPool>,
    user_claims: UserClaims,
) -> Result<HttpResponse, Error> {
    const TARGET: &str = "POST /privilege";
    log::info!(target: TARGET, "Request received");

    if user_claims.role < Role::Admin {
        log::info!(target: TARGET, "Forbidden");
        return Err(Error::new(
            Reason::Forbidden,
            "You have no permission to access this service".to_string(),
        ));
    }

    let conn = &mut pool.get()?;

    let privilege = privilege.into_inner();

    use self::users::dsl::*;

    diesel::update(users.filter(user_name.eq(privilege.username)))
        .set(user_role.eq(privilege.role))
        .execute(conn)?;

    log::info!(target: TARGET, "Request done");
    Ok(HttpResponse::Ok().finish())
}
