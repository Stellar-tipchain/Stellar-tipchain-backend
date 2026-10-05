pub mod creators;
pub mod health;
pub mod tips;

use axum::Router;

use crate::db::connection::AppState;

pub fn create_router() -> Router<AppState> {
    Router::new()
        .merge(health::routes())
        .merge(creators::routes())
        .merge(tips::routes())
}