use crate::schema::{note_hierarchy, notes};
use crate::tables::{NewNote, NewNoteHierarchy, Note, NoteHierarchy};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
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
pub struct HierarchyMapping {
    pub child_id: i32,
    pub parent_id: Option<i32>,
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
        .route("/notes/hierarchy", get(get_hierarchy_mappings))
        .route("/notes/hierarchy/attach", post(attach_child_note))
        .route(
            "/notes/hierarchy/detach/:child_id",
            delete(detach_child_note),
        )
        .with_state(state)
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

    if existing_entry.is_some() {
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

async fn get_note_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteTreeNode>>, StatusCode> {
    use crate::schema::note_hierarchy::dsl::*;
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all notes
    let all_notes: Vec<Note> = notes
        .load::<Note>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all hierarchies
    let hierarchies: Vec<NoteHierarchy> = note_hierarchy
        .load::<NoteHierarchy>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create a map of parent_id to children
    let mut parent_to_children: HashMap<Option<i32>, Vec<(i32, Option<String>)>> = HashMap::new();

    // Track which notes are children
    let mut child_notes: HashSet<i32> = HashSet::new();

    // Build the parent-to-children mapping
    for hierarchy in hierarchies {
        if let Some(child_id) = hierarchy.child_note_id {
            parent_to_children
                .entry(hierarchy.parent_note_id)
                .or_default()
                .push((child_id, hierarchy.hierarchy_type));
            child_notes.insert(child_id);
        }
    }

    // Function to recursively build the tree
    fn build_tree(
        note_id: i32,
        notes_map: &HashMap<i32, &Note>,
        parent_to_children: &HashMap<Option<i32>, Vec<(i32, Option<String>)>>,
    ) -> NoteTreeNode {
        let note = notes_map.get(&note_id).unwrap();
        let children = parent_to_children
            .get(&Some(note_id))
            .map(|children| {
                children
                    .iter()
                    .map(|(child_id, h_type)| {
                        let mut child = build_tree(*child_id, notes_map, parent_to_children);
                        child.hierarchy_type = h_type.clone();
                        child
                    })
                    .collect()
            })
            .unwrap_or_default();

        NoteTreeNode {
            id: note.id,
            title: note.title.clone(),
            created_at: note.created_at,
            modified_at: note.modified_at,
            hierarchy_type: None,
            children,
        }
    }

    // Create a map of note id to note for easy lookup
    let notes_map: HashMap<_, _> = all_notes.iter().map(|note| (note.id, note)).collect();

    // Build trees starting from root notes (notes that aren't children)
    let mut tree: Vec<NoteTreeNode> = all_notes
        .iter()
        .filter(|note| !child_notes.contains(&note.id))
        .map(|note| build_tree(note.id, &notes_map, &parent_to_children))
        .collect();

    // Sort the tree by note ID for consistent ordering
    tree.sort_by_key(|node| node.id);

    Ok(Json(tree))
}

async fn get_hierarchy_mappings(
    State(state): State<AppState>,
) -> Result<Json<Vec<HierarchyMapping>>, StatusCode> {
    use crate::schema::note_hierarchy::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mappings = note_hierarchy
        .select((child_note_id, parent_note_id, hierarchy_type))
        .load::<(Option<i32>, Option<i32>, Option<String>)>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<HierarchyMapping> = mappings
        .into_iter()
        .filter_map(|(child, parent, h_type)| {
            child.map(|c| HierarchyMapping {
                child_id: c,
                parent_id: parent,
                hierarchy_type: h_type,
            })
        })
        .collect();

    Ok(Json(response))
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

pub async fn update_database_from_notetreenode(
    State(state): State<AppState>,
    Json(note_tree_node): Json<NoteTreeNode>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Recursive function to process each node
    fn process_node(
        conn: &mut PgConnection,
        node: NoteTreeNode,
        parent_id: Option<i32>,
    ) -> Result<i32, DieselError> {
        use crate::schema::note_hierarchy::dsl::{
            child_note_id, hierarchy_type, note_hierarchy, parent_note_id,
        };
        use crate::schema::notes::dsl::{
            content, created_at, id as notes_id, modified_at, notes, title,
        };
        // Determine if the note is new or existing
        let node_id = if node.id <= 0 {
            // Insert new note
            let new_note = NewNote {
                title: &node.title,
                content: "", // Add content if provided
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };
            diesel::insert_into(notes)
                .values(&new_note)
                .returning(notes_id)
                .get_result::<i32>(conn)?
        } else {
            // Update existing note
            diesel::update(notes.filter(notes_id.eq(node.id)))
                .set((
                    title.eq(&node.title),
                    modified_at.eq(Some(chrono::Utc::now().naive_utc())),
                ))
                .execute(conn)?;
            node.id
        };

        // Update hierarchy
        // Remove existing hierarchy entry for this node
        diesel::delete(note_hierarchy.filter(child_note_id.eq(node_id))).execute(conn)?;

        // Insert new hierarchy entry if there is a parent
        if let Some(p_id) = parent_id {
            let new_hierarchy = NewNoteHierarchy {
                child_note_id: Some(node_id),
                parent_note_id: Some(p_id),
                hierarchy_type: node.hierarchy_type.as_deref(),
            };
            diesel::insert_into(note_hierarchy)
                .values(&new_hierarchy)
                .execute(conn)?;
        }

        // Process child nodes recursively
        for child in node.children {
            process_node(conn, child, Some(node_id))?;
        }

        Ok(node_id)
    }

    // Start the recursive processing from the root node
    process_node(&mut conn, note_tree_node, None).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::Json;
    use diesel::prelude::*;
    use diesel::r2d2::{ConnectionManager, Pool};
    use dotenv::dotenv;
    use std::sync::Arc;

    fn setup_test_state() -> AppState {
        dotenv().ok();
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        let pool = Pool::builder()
            .build(manager)
            .expect("Failed to create pool.");
        AppState {
            pool: Arc::new(pool),
        }
    }

    #[tokio::test]
    async fn test_update_database_from_notetreenode() {
        // Set up the test state
        let state = setup_test_state();

        // Verify the database state
        let mut conn = state.pool.get().unwrap();

        // Hierarchies before
        let hierarchies_in_db = note_hierarchy.load::<NoteHierarchy>(&mut conn).unwrap();

        // Drop the connection
        drop(conn);

        // Probably safe to assume that these IDs are unique
        let id_root = "root--lklkklklkljdkdkdkdieieieieiwiwk329032903290329032903290";
        let id_1 = "001--111idkdkcci382902192j2kj2adidsidsikk218cke91";
        let id_2 = "002--2222idskldkdkl21908210921092109219021903210921";

        // Create an input NoteTreeNode with new and existing notes
        let input_tree = NoteTreeNode {
            id: 0, // Zero or any negative number indicates a new note
            title: id_root.to_string(),
            created_at: None,
            modified_at: None,
            hierarchy_type: None,
            children: vec![
                NoteTreeNode {
                    id: 0, // New child note
                    title: id_1.to_string(),
                    created_at: None,
                    modified_at: None,
                    hierarchy_type: Some("block".to_string()),
                    children: vec![],
                },
                NoteTreeNode {
                    id: 0, // New child note
                    title: id_2.to_string(),
                    created_at: None,
                    modified_at: None,
                    hierarchy_type: Some("block".to_string()),
                    children: vec![],
                },
            ],
        };

        // Call the function to update the database
        let response =
            update_database_from_notetreenode(State(state.clone()), Json(input_tree)).await;

        // Give the database a moment to process this
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Assert that the operation was successful
        assert_eq!(response.unwrap(), StatusCode::OK);

        // Check that the notes have been added
        use crate::schema::notes::dsl::*;
        // Reset the connection
        let mut conn = state.pool.get().expect("Failed to get DB connection");
        let notes_in_db = notes.load::<Note>(&mut conn).unwrap();
        drop(conn);
        // Look through the notes for those ids
        let note_1 = notes_in_db
            .iter()
            .find(|note| note.title == id_1)
            .expect("Unable to find note 1");
        let note_2 = notes_in_db
            .iter()
            .find(|note| note.title == id_2)
            .expect("Unable to find note 2");
        assert_eq!(note_1.title, id_1);
        assert_eq!(note_2.title, id_2);
        let note_root = notes_in_db
            .iter()
            .find(|note| note.title == id_root)
            .expect("Unable to find root note");

        // Verify that the parent_id of note_1 is the root note
        use crate::schema::note_hierarchy::dsl::*;
        let mut conn = state.pool.get().expect("Failed to get DB connection");
        let hierarchies_in_db_after = note_hierarchy.load::<NoteHierarchy>(&mut conn).unwrap();
        // Find the parent id with a child id of note_1.id
        dbg!(format!("Note 1 ID: {}", note_1.id));
        dbg!(format!("Note 2 ID: {}", note_2.id));
        dbg!(format!("Root Note ID: {}", note_root.id));
        dbg!(format!("Hierarchies in DB: {:?}", hierarchies_in_db_after));
        let parent_id = hierarchies_in_db
            .iter()
            .find(|h| h.child_note_id == Some(note_1.id))
            .expect("Unable to find hierarchy entry for note 1")
            .parent_note_id
            .expect("Parent ID should not be null");
        assert_eq!(parent_id, note_root.id);

        // Check the notes have been added
        let notes_in_db_after = notes.load::<Note>(&mut conn).unwrap();
        assert_eq!(notes_in_db_after.len(), notes_in_db.len() + 2);

        // Check that the hierarchy mappings have been added
        let hierarchies_in_db = note_hierarchy.load::<NoteHierarchy>(&mut conn).unwrap();
        assert_eq!(hierarchies_in_db.len(), hierarchies_in_db_after.len() - 2);

        // Clean up by removing these
        vec![note_1, note_2, note_root].iter().for_each(|note| {
            diesel::delete(note_hierarchy.filter(child_note_id.eq(note.id)))
                .execute(&mut conn)
                .unwrap();
        });
    }
}
