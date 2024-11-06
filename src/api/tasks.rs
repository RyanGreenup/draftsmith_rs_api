use super::hierarchy::tasks::{
    attach_child_task, detach_child_task, get_hierarchy_mappings, get_task_tree,
};
use super::AppState;
use crate::schema::tasks::{self, dsl::*};
use crate::tables::{NewTask, Task};
use crate::TASK_API;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
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
    pub note_id: Option<i32>,
    pub status: String,
    pub effort_estimate: Option<BigDecimal>,
    pub actual_effort: Option<BigDecimal>,
    pub deadline: Option<NaiveDateTime>,
    pub priority: Option<i32>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<i32>,
}

#[derive(Deserialize, AsChangeset, Default)]
#[diesel(table_name = tasks)]
pub struct UpdateTaskRequest {
    pub note_id: Option<i32>,
    pub status: Option<String>,
    pub effort_estimate: Option<BigDecimal>,
    pub actual_effort: Option<BigDecimal>,
    pub deadline: Option<NaiveDateTime>,
    pub priority: Option<i32>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<i32>,
}

#[derive(Serialize)]
pub struct TaskResponse {
    pub id: i32,
    pub note_id: Option<i32>,
    pub status: String,
    pub effort_estimate: Option<BigDecimal>,
    pub actual_effort: Option<BigDecimal>,
    pub deadline: Option<NaiveDateTime>,
    pub priority: Option<i32>,
    pub created_at: Option<NaiveDateTime>,
    pub modified_at: Option<NaiveDateTime>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<i32>,
}

impl From<Task> for TaskResponse {
    fn from(task: Task) -> Self {
        Self {
            id: task.id,
            note_id: task.note_id,
            status: task.status,
            effort_estimate: task.effort_estimate,
            actual_effort: task.actual_effort,
            deadline: task.deadline,
            priority: task.priority,
            created_at: task.created_at,
            modified_at: task.modified_at,
            all_day: task.all_day,
            goal_relationship: task.goal_relationship,
        }
    }
}

async fn list_tasks(State(state): State<AppState>) -> Result<Json<Vec<TaskResponse>>, TaskError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;
    let results = tasks
        .load::<Task>(&mut conn)
        .map_err(TaskError::DatabaseError)?;
    Ok(Json(results.into_iter().map(TaskResponse::from).collect()))
}

async fn get_task(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<Json<TaskResponse>, TaskError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;
    let task = tasks
        .find(task_id)
        .first::<Task>(&mut conn)
        .map_err(|err| match err {
            diesel::result::Error::NotFound => TaskError::NotFound,
            _ => TaskError::DatabaseError(err),
        })?;
    Ok(Json(TaskResponse::from(task)))
}

async fn create_task(
    State(state): State<AppState>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TaskResponse>), TaskError> {
    let new_task = NewTask {
        note_id: payload.note_id,
        status: &payload.status,
        effort_estimate: payload.effort_estimate,
        actual_effort: payload.actual_effort,
        deadline: payload.deadline,
        priority: payload.priority,
        created_at: Some(chrono::Utc::now().naive_utc()),
        modified_at: Some(chrono::Utc::now().naive_utc()),
        all_day: payload.all_day,
        goal_relationship: payload.goal_relationship,
    };
    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;
    let task = diesel::insert_into(tasks::table)
        .values(&new_task)
        .get_result::<Task>(&mut conn)
        .map_err(TaskError::DatabaseError)?;
    Ok((StatusCode::CREATED, Json(TaskResponse::from(task))))
}

async fn update_task(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<TaskResponse>, TaskError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| TaskError::InternalServerError)?;
    let updated_task = diesel::update(tasks.find(task_id))
        .set(payload)
        .get_result::<Task>(&mut conn)
        .map_err(|err| match err {
            diesel::result::Error::NotFound => TaskError::NotFound,
            _ => TaskError::DatabaseError(err),
        })?;
    Ok(Json(TaskResponse::from(updated_task)))
}

async fn delete_task(
    State(state): State<AppState>,
    Path(task_id): Path<i32>,
) -> Result<StatusCode, TaskError> {
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

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route(
            format!("/{TASK_API}").as_str(),
            get(list_tasks).post(create_task),
        )
        .route(
            format!("/{TASK_API}/:id").as_str(),
            get(get_task).put(update_task).delete(delete_task),
        )
        .route(format!("/{TASK_API}/tree").as_str(), get(get_task_tree))
        .route(
            format!("/{TASK_API}/hierarchy").as_str(),
            get(get_hierarchy_mappings),
        )
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
    use axum::extract::{Path, State};
    use axum::Json;
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::PgConnection;
    use dotenv::dotenv;
    use std::env;
    use std::sync::Arc;

    fn setup_test_state() -> AppState {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        let pool = r2d2::Pool::builder()
            .max_size(5)
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
                note_id: None,
                status: "todo".to_string(),
                effort_estimate: None,
                actual_effort: None,
                deadline: None,
                priority: Some(1),
                all_day: Some(false),
                goal_relationship: None,
            }),
        )
        .await
        .expect("Failed to create task");
        let task_id = create_response.1 .0.id;

        // Test get
        let get_response = get_task(State(state.clone()), Path(task_id))
            .await
            .expect("Failed to get task");
        assert_eq!(get_response.0.status, "todo");

        // Test update
        let update_response = update_task(
            State(state.clone()),
            Path(task_id),
            Json(UpdateTaskRequest {
                status: Some("done".to_string()),
                ..Default::default()
            }),
        )
        .await
        .expect("Failed to update task");
        assert_eq!(update_response.0.status, "done");

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
