use super::hierarchy::tags::{
    attach_child_tag, detach_child_tag, get_hierarchy_mappings, get_tag_tree,
};
use super::AppState;
pub use crate::tables::{NewTag, Tag, NoteTag, NewNoteTag};
use crate::schema::note_tags;
use crate::TAGS_API;
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
pub enum TagError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] diesel::result::Error),

    #[error("Tag not found")]
    NotFound,

    #[error("Internal server error")]
    InternalServerError,
}

impl IntoResponse for TagError {
    fn into_response(self) -> Response {
        let status_code = match self {
            TagError::DatabaseError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            TagError::NotFound => StatusCode::NOT_FOUND,
            TagError::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, self.to_string()).into_response()
    }
}

#[derive(Deserialize)]
pub struct CreateTagRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct UpdateTagRequest {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
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

#[derive(Deserialize)]
pub struct AttachTagRequest {
    pub note_id: i32,
    pub tag_id: i32,
}

#[derive(Serialize)]
pub struct NoteTagResponse {
    pub note_id: i32,
    pub tag_id: i32,
}

impl From<NoteTag> for NoteTagResponse {
    fn from(note_tag: NoteTag) -> Self {
        Self {
            note_id: note_tag.note_id,
            tag_id: note_tag.tag_id,
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
        .route(format!("/{TAGS_API}/tree").as_str(), get(get_tag_tree))
        .route(
            format!("/{TAGS_API}/notes").as_str(),
            get(list_note_tags).post(attach_tag_to_note),
        )
        .route(
            format!("/{TAGS_API}/notes/:note_id/:tag_id").as_str(),
            delete(detach_tag_from_note),
        )
        .route(
            format!("/{TAGS_API}/hierarchy").as_str(),
            get(get_hierarchy_mappings),
        )
        .route(
            format!("/{TAGS_API}/hierarchy/attach").as_str(),
            post(attach_child_tag),
        )
        .route(
            format!("/{TAGS_API}/hierarchy/detach/:id").as_str(),
            delete(detach_child_tag),
        )
}

async fn list_tags(State(state): State<AppState>) -> Result<Json<Vec<TagResponse>>, TagError> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let results = tags
        .load::<Tag>(&mut conn)
        .map_err(TagError::DatabaseError)?;

    Ok(Json(results.into_iter().map(Into::into).collect()))
}

async fn get_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<i32>,
) -> Result<Json<TagResponse>, TagError> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let tag = tags
        .find(tag_id)
        .first::<Tag>(&mut conn)
        .map_err(|err| match err {
            diesel::result::Error::NotFound => TagError::NotFound,
            _ => TagError::DatabaseError(err),
        })?;

    Ok(Json(tag.into()))
}

async fn create_tag(
    State(state): State<AppState>,
    Json(payload): Json<CreateTagRequest>,
) -> Result<(StatusCode, Json<TagResponse>), TagError> {
    use crate::schema::tags;

    let new_tag = NewTag {
        name: &payload.name,
    };

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let tag = diesel::insert_into(tags::table)
        .values(&new_tag)
        .get_result::<Tag>(&mut conn)
        .map_err(TagError::DatabaseError)?;

    Ok((StatusCode::CREATED, Json(tag.into())))
}

async fn update_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<i32>,
    Json(payload): Json<UpdateTagRequest>,
) -> Result<Json<TagResponse>, TagError> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let tag = diesel::update(tags.find(tag_id))
        .set(name.eq(payload.name))
        .get_result::<Tag>(&mut conn)
        .map_err(|err| match err {
            diesel::result::Error::NotFound => TagError::NotFound,
            _ => TagError::DatabaseError(err),
        })?;

    Ok(Json(tag.into()))
}

async fn delete_tag(
    State(state): State<AppState>,
    Path(tag_id): Path<i32>,
) -> Result<StatusCode, TagError> {
    use crate::schema::tags::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let result = diesel::delete(tags.find(tag_id))
        .execute(&mut conn)
        .map_err(TagError::DatabaseError)?;

    if result > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(TagError::NotFound)
    }
}

async fn list_note_tags(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteTagResponse>>, TagError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let results = note_tags::table
        .load::<NoteTag>(&mut conn)
        .map_err(TagError::DatabaseError)?;

    Ok(Json(results.into_iter().map(Into::into).collect()))
}

async fn attach_tag_to_note(
    State(state): State<AppState>,
    Json(payload): Json<AttachTagRequest>,
) -> Result<(StatusCode, Json<NoteTagResponse>), TagError> {
    let new_note_tag = NewNoteTag {
        note_id: payload.note_id,
        tag_id: payload.tag_id,
    };

    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let note_tag = diesel::insert_into(note_tags::table)
        .values(&new_note_tag)
        .get_result::<NoteTag>(&mut conn)
        .map_err(TagError::DatabaseError)?;

    Ok((StatusCode::CREATED, Json(note_tag.into())))
}

async fn detach_tag_from_note(
    State(state): State<AppState>,
    Path((note_id, tag_id)): Path<(i32, i32)>,
) -> Result<StatusCode, TagError> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| TagError::InternalServerError)?;

    let result = diesel::delete(
        note_tags::table
            .filter(note_tags::note_id.eq(note_id))
            .filter(note_tags::tag_id.eq(tag_id)),
    )
    .execute(&mut conn)
    .map_err(TagError::DatabaseError)?;

    if result > 0 {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(TagError::NotFound)
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
        assert!(matches!(get_result, Err(TagError::NotFound)));
    }
}
