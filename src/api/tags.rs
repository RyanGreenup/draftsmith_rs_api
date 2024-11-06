use super::AppState;
use crate::tables::{NewTag, Tag};
use crate::TAGS_API;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateTagRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
}

impl From<Tag> for TagResponse {
    fn from(tag: Tag) -> Self {
        Self {
            id: tag.id,
            name: tag.name,
        }
    }
}

pub fn create_router() -> Router<AppState> {
    Router::new()
        .route(
            format!("/{TAGS_API}").as_str(),
            get(list_tags).post(create_tag),
        )
        .route(
            format!("/{TAGS_API}/:id").as_str(),
            get(get_tag).put(update_tag).delete(delete_tag),
        )
        .route(
            format!("/{TAGS_API}/tree").as_str(),
            get(super::hierarchy::tags::get_tag_tree),
        )
        .route(
            format!("/{TAGS_API}/attach").as_str(),
            post(super::hierarchy::tags::attach_child_tag),
        )
        .route(
            format!("/{TAGS_API}/detach/:id").as_str(),
            delete(super::hierarchy::tags::detach_child_tag),
        )
}

async fn list_tags(State(state): State<AppState>) -> Result<Json<Vec<TagResponse>>, StatusCode> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results = tags
        .load::<Tag>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(results.into_iter().map(Into::into).collect()))
}

async fn get_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<i32>,
) -> Result<Json<TagResponse>, StatusCode> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tag = tags
        .find(tag_id)
        .first::<Tag>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(tag.into()))
}

async fn create_tag(
    State(state): State<AppState>,
    Json(payload): Json<CreateTagRequest>,
) -> Result<(StatusCode, Json<TagResponse>), StatusCode> {
    use crate::schema::tags;

    let new_tag = NewTag {
        name: &payload.name,
    };

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tag = diesel::insert_into(tags::table)
        .values(&new_tag)
        .get_result::<Tag>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(tag.into())))
}

async fn update_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<i32>,
    Json(payload): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, StatusCode> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tag = diesel::update(tags.find(tag_id))
        .set(name.eq(payload.name))
        .get_result::<Tag>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(tag.into()))
}

async fn delete_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result = diesel::delete(tags.find(tag_id))
        .execute(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if result > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(StatusCode::NOT_FOUND)
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
    async fn test_tag_crud() {
        let state = setup_test_state();

        // Test create
        let create_response = create_tag(
            State(state.clone()),
            Json(CreateTagRequest {
                name: "Test Tag".to_string(),
            }),
        )
        .await
        .expect("Failed to create tag");

        let tag_id = create_response.1 .0.id;

        // Test get
        let get_response = get_tag(State(state.clone()), Path(tag_id))
            .await
            .expect("Failed to get tag");
        assert_eq!(get_response.0.name, "Test Tag");

        // Test update
        let update_response = update_tag(
            State(state.clone()),
            Path(tag_id),
            Json(UpdateTagRequest {
                name: "Updated Tag".to_string(),
            }),
        )
        .await
        .expect("Failed to update tag");
        assert_eq!(update_response.0.name, "Updated Tag");

        // Test delete
        let delete_response = delete_tag(State(state.clone()), Path(tag_id))
            .await
            .expect("Failed to delete tag");
        assert_eq!(delete_response, StatusCode::NO_CONTENT);

        // Verify deletion
        let get_result = get_tag(State(state), Path(tag_id)).await;
        assert!(get_result.is_err());
    }
}
