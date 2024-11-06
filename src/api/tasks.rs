use super::hierarchy::tasks::{attach_child_task, detach_child_task, get_task_tree};
use super::AppState;
use crate::tables::{task, Newtask};
use crate::TASKS_API;
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
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateTaskRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: i32,
    pub name: String,
}

impl From<task> for TaskResponse {
    fn from(task: task) -> Self {
        Self {
            id: task.id,
            name: task.name,
        }
    }
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route(
            format!("/{TASKS_API}").as_str(),
            get(list_tasks).post(create_task),
        )
        .route(
            format!("/{TASKS_API}/:id").as_str(),
            get(get_task).put(update_task).delete(delete_task),
        )
        .route(format!("/{TASKS_API}/tree").as_str(), get(get_task_tree))
        .route(
            format!("/{TASKS_API}/hierarchy/attach").as_str(),
            post(attach_child_task),
        )
        .route(
            format!("/{TASKS_API}/hierarchy/detach/:id").as_str(),
            delete(detach_child_task),
        )
        .route(format!("/{TASKS_API}/tree").as_str(), get(get_task_tree))
        .route(
            format!("/{TASKS_API}/attach").as_str(),
            post(attach_child_task),
        )
        .route(
            format!("/{TASKS_API}/detach/:id").as_str(),
            delete(detach_child_task),
        )
}

async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<TaskResponse>>, TaskError> {
    use crate::schema::tasks::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;

    let results = tasks
        .load::<task>(&mut conn)
        .map_err(TaskError::DatabaseError)?;

    Ok(Json(results.into_iter().map(Into::into).collect()))
}

async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<Json<TaskResponse>, TaskError> {
    use crate::schema::tasks::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;

    let task = tasks
        .find(task_id)
        .first::<task>(&mut conn)
        .map_err(|err| match err {
            diesel::result::Error::NotFound => TaskError::NotFound,
            _ => TaskError::DatabaseError(err),
        })?;

    Ok(Json(task.into()))
}

async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), TaskError> {
    use crate::schema::tasks;

    let new_task = Newtask {
        name: &payload.name,
    };

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;

    let task = diesel::insert_into(tasks::table)
        .values(&new_task)
        .get_result::<task>(&mut conn)
        .map_err(TaskError::DatabaseError)?;

    Ok((StatusCode::CREATED, Json(task.into())))
}

async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, TaskError> {
    use crate::schema::tasks::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;

    let task = diesel::update(tasks.find(task_id))
        .set(name.eq(payload.name))
        .get_result::<task>(&mut conn)
        .map_err(|err| match err {
            diesel::result::Error::NotFound => TaskError::NotFound,
            _ => TaskError::DatabaseError(err),
        })?;

    Ok(Json(task.into()))
}

async fn delete_task(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<StatusCode, TaskError> {
    use crate::schema::tasks::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;

    let result = diesel::delete(tasks.find(task_id))
        .execute(&mut conn)
        .map_err(TaskError::DatabaseError)?;

    if result > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(TaskError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::PgConnection;
    use std::sync::Arc;

    fn setup_test_state() -> AppState {
        dotenv::dotenv().ok();
        let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        let pool = r2d2::Pool::builder()
            .build(manager)
            .expect("Failed to create pool.");
        AppState {
            pool: Arc::new(pool),
        }
    }

    #[tokio::test]
    async fn test_task_crud() {
        let state = setup_test_state();

        // Test create
        let create_response = create_task(
            State(state.clone()),
            Json(CreateTaskRequest {
                name: "Test task".to_string(),
            }),
        )
        .await
        .expect("Failed to create task");

        let task_id = create_response.1 .0.id;

        // Test get
        let get_response = get_task(State(state.clone()), Path(task_id))
            .await
            .expect("Failed to get task");
        assert_eq!(get_response.0.name, "Test task");

        // Test update
        let update_response = update_task(
            State(state.clone()),
            Path(task_id),
            Json(UpdateTaskRequest {
                name: "Updated task".to_string(),
            }),
        )
        .await
        .expect("Failed to update task");
        assert_eq!(update_response.0.name, "Updated task");

        // Test delete
        let delete_response = delete_task(State(state.clone()), Path(task_id))
            .await
            .expect("Failed to delete task");
        assert_eq!(delete_response, StatusCode::NO_CONTENT);

        // Verify deletion
        let get_result = get_task(State(state), Path(task_id)).await;
        assert!(matches!(get_result, Err(TaskError::NotFound)));
    }
}
