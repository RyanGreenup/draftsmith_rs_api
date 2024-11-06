use super::hierarchy::tasks::{attach_child_task, detach_child_task, get_task_tree};
use crate::TASK_API;
use super::AppState;
use crate::schema::tasks::dsl::*;
use crate::tables::{Task, NewTask};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),

    #[error("task not found")]
    NotFound,

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for TaskError {
    fn into_response(self) -> Response {
        let status_code = match self {
            TaskError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            TaskError::NotFound => StatusCode::NOT_FOUND,
            TaskError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    // TODO
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    // TODO
}

#[derive(Serialize)]
pub struct TaskResponse {
    // TODO
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        // TODO
    }
}


pub fn create_router() -> Router<AppState> {
    Router::new()
        .route(format!("/{TASK_API}").as_str(), get(list_tasks).post(create_task))
        .route(
            format!("/{TASK_API}/:id").as_str(),
            get(get_task).put(update_task).delete(delete_task),
        )
        .route(format!("/{TASK_API}/tree").as_str(), get(get_task_tree))
        .route(
            format!("/{TASK_API}/hierarchy/attach").as_str(),
            post(attach_child_task),
        )
        .route(
            format!("/{TASK_API}/hierarchy/detach/:id").as_str(),
            delete(detach_child_task),
        )
}


#[cfg(test)]
mod tests {
    use super::*;
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::PgConnection;
    use std::sync::Arc;
    use super::tests::setup_test_state;


}
