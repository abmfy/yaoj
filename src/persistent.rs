use diesel::prelude::*;

pub mod models;
pub mod schema;

pub fn establish_connection() -> SqliteConnection {
    const DATABASE_URL: &str = "oj.db";
    SqliteConnection::establish(DATABASE_URL).expect("Unable to establish database connection")
}
