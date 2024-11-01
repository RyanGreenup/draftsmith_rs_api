use crate::tables::{NewNote, Note};
use axum::{
    extract::{Path, State, Query},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::{json, Value};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// Connection pool type
type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;

// Shared state
#[derive(Clone)]
pub struct AppState {
    pool: Arc<Pool>,
}

// Request/Response types
#[derive(Deserialize)]
pub struct CreateNoteRequest {
    title: String,
    content: String,
}

#[derive(Deserialize)]
pub struct UpdateNoteRequest {
    title: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
pub struct NoteResponse {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
}

#[derive(Serialize, Deserialize)]
pub struct NoteMetadataResponse {
    pub id: i32,
    pub title: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
}

#[derive(Serialize)]
pub struct NoteTreeNode {
    pub id: i32,
    pub title: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub hierarchy_type: Option<String>,
    pub children: Vec<NoteTreeNode>,
}

impl From<Note> for NoteResponse {
    fn from(note: Note) -> Self {
        Self {
            id: note.id,
            title: note.title,
            content: note.content,
            created_at: note.created_at,
            modified_at: note.modified_at,
        }
    }
}

pub fn create_router(pool: Pool) -> Router {
    let state = AppState {
        pool: Arc::new(pool),
    };

    Router::new()
        .route("/notes/flat", get(list_notes).post(create_note))
        .route(
            "/notes/flat/:id",
            get(get_note).put(update_note).delete(delete_note),
        )
        .route("/notes/tree", get(get_note_tree))
        .with_state(state)
}

async fn get_note_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteTreeNode>>, StatusCode> {
    use crate::schema::{notes, note_hierarchy};
    use diesel::dsl::count_star;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if there are any hierarchy entries
    let hierarchy_count: i64 = note_hierarchy::table
        .select(count_star())
        .first(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if hierarchy_count == 0 {
        // If no hierarchy exists, return all notes in a linear fashion
        let all_notes = notes::table
            .load::<crate::tables::Note>(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let tree_nodes: Vec<NoteTreeNode> = all_notes
            .into_iter()
            .map(|note| NoteTreeNode {
                id: note.id,
                title: note.title,
                created_at: note.created_at,
                modified_at: note.modified_at,
                hierarchy_type: None,
                children: Vec::new(),
            })
            .collect();

        return Ok(Json(tree_nodes));
    }

    // Helper function to recursively build the tree
    fn build_tree(
        conn: &mut PgConnection,
        parent_id: Option<i32>,
    ) -> Result<Vec<NoteTreeNode>, diesel::result::Error> {
        // Get all child notes for this parent
        let children = note_hierarchy::table
            .filter(note_hierarchy::parent_note_id.eq(parent_id))
            .inner_join(notes::table.on(notes::id.eq(note_hierarchy::child_note_id.assume_not_null())))
            .load::<(crate::tables::NoteHierarchy, crate::tables::Note)>(conn)?;

        let mut tree_nodes = Vec::new();

        for (hierarchy, note) in children {
            // Recursively get children for this node
            let child_nodes = build_tree(conn, Some(note.id))?;

            tree_nodes.push(NoteTreeNode {
                id: note.id,
                title: note.title,
                created_at: note.created_at,
                modified_at: note.modified_at,
                hierarchy_type: hierarchy.hierarchy_type,
                children: child_nodes,
            });
        }

        Ok(tree_nodes)
    }

    // Start building the tree from root nodes (those with no parent)
    let mut root_nodes = build_tree(&mut conn, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If we got no root nodes but we know hierarchies exist,
    // there might be orphaned notes. Add them as root nodes.
    if root_nodes.is_empty() {
        let orphaned_notes = notes::table
            .left_outer_join(note_hierarchy::table.on(
                notes::id.eq(note_hierarchy::child_note_id.assume_not_null())
            ))
            .filter(note_hierarchy::id.is_null())
            .select(notes::all_columns)
            .load::<crate::tables::Note>(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        root_nodes.extend(orphaned_notes.into_iter().map(|note| NoteTreeNode {
            id: note.id,
            title: note.title,
            created_at: note.created_at,
            modified_at: note.modified_at,
            hierarchy_type: None,
            children: Vec::new(),
        }));
    }

    Ok(Json(root_nodes))
}


#[derive(Deserialize)]
pub struct ListNotesParams {
    #[serde(default)]
    exclude_content: bool,
}

async fn list_notes(
    State(state): State<AppState>,
    Query(params): Query<ListNotesParams>,
) -> Result<Json<Value>, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results = notes
        .load::<Note>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if params.exclude_content {
        let response: Vec<NoteMetadataResponse> = results
            .into_iter()
            .map(|note| NoteMetadataResponse {
                id: note.id,
                title: note.title,
                created_at: note.created_at,
                modified_at: note.modified_at,
            })
            .collect();
        Ok(Json(json!(response)))
    } else {
        let response: Vec<NoteResponse> = results.into_iter().map(Into::into).collect();
        Ok(Json(json!(response)))
    }
}

async fn get_note(
    Path(note_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<Json<NoteResponse>, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let note = notes
        .find(note_id)
        .first::<Note>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(note.into()))
}

async fn update_note(
    Path(note_id): Path<i32>,
    State(state): State<AppState>,
    Json(payload): Json<UpdateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let updated_note = diesel::update(notes.find(note_id))
        .set((
            title.eq(payload.title),
            content.eq(payload.content),
            modified_at.eq(Some(chrono::Utc::now().naive_utc())),
        ))
        .get_result::<Note>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((StatusCode::OK, Json(updated_note.into())))
}

#[derive(Serialize)]
struct DeleteResponse {
    message: String,
    deleted_id: i32,
}

async fn delete_note(
    Path(note_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result = diesel::delete(notes.find(note_id))
        .execute(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    if result > 0 {
        let response = DeleteResponse {
            message: format!("Note {} successfully deleted", note_id),
            deleted_id: note_id,
        };
        Ok((StatusCode::OK, Json(response)))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn create_note(
    State(state): State<AppState>,
    Json(payload): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteResponse>), StatusCode> {
    use crate::schema::notes;

    let new_note = NewNote {
        title: &payload.title,
        content: &payload.content,
        created_at: Some(chrono::Utc::now().naive_utc()),
        modified_at: Some(chrono::Utc::now().naive_utc()),
    };

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let note = diesel::insert_into(notes::table)
        .values(&new_note)
        .get_result::<Note>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(note.into())))
}
