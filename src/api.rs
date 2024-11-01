use crate::tables::{NewNote, NewNoteHierarchy, Note, NoteHierarchy};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use diesel::prelude::*;
use std::collections::{HashMap, HashSet};
use diesel::r2d2::{self, ConnectionManager};
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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

#[derive(Deserialize)]
pub struct AttachChildRequest {
    pub child_note_id: i32,
    pub parent_note_id: Option<i32>,
    pub hierarchy_type: Option<String>,
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
        .route("/notes/hierarchy/attach", post(attach_child_note))
        .route(
            "/notes/hierarchy/detach/:child_id",
            delete(detach_child_note),
        )
        .with_state(state)
}

async fn get_note_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteTreeNode>>, StatusCode> {
    use crate::schema::{note_hierarchy, notes};
    use std::collections::{HashMap, HashSet};

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch all notes
    let all_notes = notes::table
        .load::<Note>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build a mapping from note IDs to Note instances
    let mut notes_map: HashMap<i32, Note> = all_notes
        .into_iter()
        .map(|note| (note.id, note))
        .collect();

    // Fetch all hierarchy entries
    let hierarchies = note_hierarchy::table
        .load::<NoteHierarchy>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Build mappings
    let mut parent_to_children: HashMap<Option<i32>, Vec<i32>> = HashMap::new();
    let mut hierarchy_types: HashMap<i32, Option<String>> = HashMap::new();

    for hierarchy in &hierarchies {
        let child_id = hierarchy.child_note_id.unwrap();
        let parent_id = hierarchy.parent_note_id;
        parent_to_children
            .entry(parent_id)
            .or_default()
            .push(child_id);
        hierarchy_types.insert(child_id, hierarchy.hierarchy_type.clone());
    }

    // Initialize NoteTreeNodes for all notes
    let mut note_nodes: HashMap<i32, NoteTreeNode> = notes_map
        .iter()
        .map(|(&id, note)| {
            let hierarchy_type = hierarchy_types.get(&id).cloned().flatten();
            (
                id,
                NoteTreeNode {
                    id,
                    title: note.title.clone(),
                    created_at: note.created_at,
                    modified_at: note.modified_at,
                    hierarchy_type,
                    children: Vec::new(),
                },
            )
        })
        .collect();

    // Build the tree by linking child nodes to their parents
    for (&parent_id_option, child_ids) in &parent_to_children {
        if let Some(parent_id) = parent_id_option {
            if let Some(parent_node) = note_nodes.get_mut(&parent_id) {
                for &child_id in child_ids {
                    if let Some(child_node) = note_nodes.get(&child_id) {
                        parent_node.children.push(child_node.clone());
                    }
                }
            }
        }
    }

    // Identify root nodes (notes without parents)
    let mut root_ids: HashSet<i32> = notes_map.keys().cloned().collect();
    for child_ids in parent_to_children.values() {
        for &child_id in child_ids {
            root_ids.remove(&child_id);
        }
    }

    // Collect root nodes, including those not in the hierarchy
    let mut root_nodes = Vec::new();
    for id in root_ids {
        if let Some(node) = note_nodes.get(&id) {
            // Add children to root nodes if they have any
            if let Some(child_ids) = parent_to_children.get(&Some(id)) {
                let children = child_ids
                    .iter()
                    .filter_map(|&cid| note_nodes.get(&cid).cloned())
                    .collect();
                let mut root_node = node.clone();
                root_node.children = children;
                root_nodes.push(root_node);
            } else {
                root_nodes.push(node.clone());
            }
        }
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

fn is_circular_hierarchy(
    conn: &mut PgConnection,
    child_id: i32,
    potential_parent_id: Option<i32>,
) -> Result<bool, DieselError> {
    use crate::schema::note_hierarchy::dsl::*;
    let mut current_parent_id = potential_parent_id;
    while let Some(pid) = current_parent_id {
        if pid == child_id {
            return Ok(true); // Circular hierarchy detected
        }
        current_parent_id = note_hierarchy
            .filter(child_note_id.eq(pid))
            .select(parent_note_id)
            .first::<Option<i32>>(conn)
            .optional()?
            .flatten();
    }
    Ok(false)
}

async fn attach_child_note(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildRequest>,
) -> Result<StatusCode, StatusCode> {
    use crate::schema::note_hierarchy::dsl::*;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prevent circular hierarchy
    if let Some(parent_id) = payload.parent_note_id {
        if is_circular_hierarchy(&mut conn, payload.child_note_id, Some(parent_id))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            return Err(StatusCode::BAD_REQUEST); // Circular hierarchy detected
        }
    }

    // Check if a hierarchy entry already exists for the child
    let existing_entry = note_hierarchy
        .filter(child_note_id.eq(payload.child_note_id))
        .first::<NoteHierarchy>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some(_) = existing_entry {
        // Update the existing hierarchy entry
        diesel::update(note_hierarchy.filter(child_note_id.eq(payload.child_note_id)))
            .set((
                parent_note_id.eq(payload.parent_note_id),
                hierarchy_type.eq(payload.hierarchy_type.clone()),
            ))
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        // Create a new hierarchy entry
        let new_entry = NewNoteHierarchy {
            child_note_id: Some(payload.child_note_id),
            parent_note_id: payload.parent_note_id,
            hierarchy_type: payload.hierarchy_type.as_deref(),
        };

        diesel::insert_into(note_hierarchy)
            .values(&new_entry)
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(StatusCode::OK)
}

async fn detach_child_note(
    State(state): State<AppState>,
    Path(child_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use crate::schema::note_hierarchy::dsl::*;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Delete the hierarchy entry for this child note
    let num_deleted = diesel::delete(note_hierarchy.filter(child_note_id.eq(child_id)))
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if num_deleted == 0 {
        return Err(StatusCode::NOT_FOUND);
    }

    Ok(StatusCode::NO_CONTENT)
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
