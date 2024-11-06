use crate::api::state::AppState;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use dotenv::dotenv;
use lazy_static::lazy_static;
use std::sync::Arc;

lazy_static! {
    static ref TEST_STATE: AppState = {
        dotenv().ok();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        let pool = Pool::builder()
            .max_size(5)
            .build(manager)
            .expect("Failed to create pool.");
        AppState {
            pool: Arc::new(pool),
        }
    };
}

pub fn setup_test_state() -> AppState {
    TEST_STATE.clone()
}
