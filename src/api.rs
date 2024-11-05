use crate::client::NoteError;
use crate::tables::{HierarchyMapping, NoteWithParent};
use crate::tables::{NewNote, NewNoteHierarchy, Note, NoteHierarchy, NoteWithoutFts};
use crate::{FLAT_API, SEARCH_FTS_API};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use axum_extra::response::ErasedJson;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::result::Error as DieselError;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
struct RenderedNote {
    id: i32,
    rendered_content: String,
}
use sha2::{Digest, Sha256};
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
#[derive(Deserialize, Serialize)]
pub struct CreateNoteRequest {
    pub title: String,
    pub content: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct UpdateNoteRequest {
    pub title: Option<String>,
    pub content: String,
}

type NoteResponse = NoteWithoutFts;

#[derive(Serialize, Deserialize)]
pub struct NoteMetadataResponse {
    pub id: i32,
    pub title: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct SearchQuery {
    q: String,
}

#[derive(Deserialize, Serialize)]
pub struct AttachChildRequest {
    pub child_note_id: i32,
    pub parent_note_id: Option<i32>,
    pub hierarchy_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NoteTreeNode {
    pub id: i32,
    pub title: Option<String>,
    pub content: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub hierarchy_type: Option<String>,
    pub children: Vec<NoteTreeNode>,
}

async fn fts_search_notes(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<NoteWithoutFts>>, StatusCode> {
    use crate::schema::notes::dsl::*;
    use diesel::dsl::sql;
    use diesel::prelude::*;
    use diesel::sql_types::{Bool, Float8};

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Convert the search query to a tsquery, escaping single quotes
    let tsquery = format!(
        "plainto_tsquery('english', '{}')",
        query.q.replace('\'', "''")
    );

    // Perform the full text search using ts_rank
    let results = notes
        .select((id, title, content, created_at, modified_at))
        .filter(sql::<Bool>(&format!("fts @@ {}", tsquery)))
        .order_by(sql::<Float8>(&format!("ts_rank(fts, {}) DESC", tsquery)))
        .load::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(results))
}

pub fn create_router(pool: Pool) -> Router {
    let state = AppState {
        pool: Arc::new(pool),
    };

    let max_body_size = 1024 * 1024 * 1024; // 1 GB

    Router::new()
        .layer(DefaultBodyLimit::max(max_body_size))
        .route(format!("/{SEARCH_FTS_API}").as_str(), get(fts_search_notes))
        .route("/notes/search/semantic", get(fts_search_notes))
        .route("/notes/search/hybrid", get(fts_search_notes))
        .route("/notes/search/typesense", get(fts_search_notes))
        .route("/notes/flat", get(list_notes).post(create_note))
        .route(
            format!("/{FLAT_API}/:id").as_str(),
            get(get_note).put(update_note).delete(delete_note),
        )
        .route("/notes/flat/:id/hash", get(get_note_hash))
        .route("/notes/flat/hashes", get(get_all_note_hashes))
        .route("/notes/flat/batch", put(update_notes))
        .route("/notes/tree", get(get_note_tree))
        .route("/notes/hierarchy", get(get_hierarchy_mappings))
        .route("/notes/hierarchy/attach", post(attach_child_note))
        .route(
            "/notes/hierarchy/detach/:child_id",
            delete(detach_child_note),
        )
        .route("/notes/tree", put(update_note_tree))
        .route("/notes/flat/:id/render/html", get(render_note_html))
        .route("/notes/flat/:id/render/md", get(render_note_md))
        .route("/notes/flat/render/html", get(render_all_notes_html))
        .route("/notes/flat/render/md", get(render_all_notes_md))
        .with_state(state)
}

#[derive(Deserialize, Serialize)]
pub struct ListNotesParams {
    #[serde(default)]
    exclude_content: bool,
}

async fn list_notes(
    State(state): State<AppState>,
    Query(params): Query<ListNotesParams>,
) -> Result<ErasedJson, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results = NoteWithoutFts::get_all(&mut conn).map_err(|_| {
        println!("An error occurred while loading notes.");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

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
        Ok(ErasedJson::pretty(response))
    } else {
        let response: Vec<NoteResponse> = results
            .into_iter()
            .map(|note| NoteResponse {
                id: note.id,
                title: note.title,
                content: note.content,
                created_at: note.created_at,
                modified_at: note.modified_at,
            })
            .collect();
        Ok(ErasedJson::pretty(response))
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

#[derive(Deserialize, Serialize)]
pub struct BatchUpdateRequest {
    pub updates: Vec<(i32, UpdateNoteRequest)>,
}

#[derive(Deserialize, Serialize)]
pub struct BatchUpdateResponse {
    pub updated: Vec<NoteResponse>,
    pub failed: Vec<i32>,
}

async fn update_single_note(
    pool: Arc<Pool>,
    note_id: i32,
    update: UpdateNoteRequest,
) -> Result<Note, DieselError> {
    use crate::schema::notes::dsl::*;

    let mut conn = pool.get().map_err(|_| DieselError::RollbackTransaction)?;
    let changes = (
        content.eq(update.content),
        modified_at.eq(Some(chrono::Utc::now().naive_utc())),
    );

    if let Some(new_title) = update.title {
        diesel::update(notes.find(note_id))
            .set((title.eq(new_title), changes))
            .get_result::<Note>(&mut conn)
    } else {
        diesel::update(notes.find(note_id))
            .set(changes)
            .get_result::<Note>(&mut conn)
    }
}

async fn update_notes(
    State(state): State<AppState>,
    Json(payload): Json<BatchUpdateRequest>,
) -> Result<Json<BatchUpdateResponse>, StatusCode> {
    let futures = payload.updates.into_iter().map(|(note_id, update)| {
        let pool = Arc::clone(&state.pool);
        async move {
            match update_single_note(pool, note_id, update).await {
                Ok(note) => (Ok(note), note_id),
                Err(_) => (Err(()), note_id),
            }
        }
    });

    let results = join_all(futures).await;

    let mut updated = Vec::new();
    let mut failed = Vec::new();

    for (result, note_id) in results {
        match result {
            Ok(note) => updated.push(note.into()),
            Err(_) => failed.push(note_id),
        }
    }

    Ok(Json(BatchUpdateResponse { updated, failed }))
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

    let changes = (
        content.eq(payload.content),
        modified_at.eq(Some(chrono::Utc::now().naive_utc())),
    );

    let updated_note = if let Some(new_title) = payload.title {
        diesel::update(notes.find(note_id))
            .set((title.eq(new_title), changes))
            .get_result::<Note>(&mut conn)
    } else {
        diesel::update(notes.find(note_id))
            .set(changes)
            .get_result::<Note>(&mut conn)
    }
    .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((StatusCode::OK, Json(updated_note.into())))
}

#[derive(Serialize, Deserialize)]
struct DeleteResponse {
    message: String,
    deleted_id: i32,
}

pub fn compute_note_hash(note: &NoteWithParent) -> String {
    // Create a string containing all note properties including parent_id
    let note_string = format!(
        "id:{},title:{},content:{},created_at:{:?},modified_at:{:?},parent_id:{:?}",
        note.note_id, note.title, note.content, note.created_at, note.modified_at, note.parent_id
    );

    // Compute hash
    let mut hasher = Sha256::new();
    hasher.update(note_string.as_bytes());
    format!("{:x}", hasher.finalize())
}

async fn get_note_hash(
    Path(note_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<String, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let note = NoteWithParent::get_by_id(&mut conn, note_id).map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(compute_note_hash(&note))
}

#[derive(Deserialize, Serialize)]
pub struct NoteHash {
    pub id: i32,
    pub hash: String,
}

pub async fn compute_all_note_hashes(
    all_notes: Vec<NoteWithParent>,
) -> Result<HashMap<i32, String>, NoteError> {
    // Process notes concurrently using tokio's spawn
    let hash_futures: Vec<_> = all_notes
        .into_iter()
        .map(|note| {
            let note_id = note.note_id;
            tokio::spawn(async move {
                Ok::<(i32, String), NoteError>((note_id, compute_note_hash(&note)))
            })
        })
        .collect();

    // Wait for all hashes to complete and collect into HashMap
    let mut note_hashes = HashMap::new();
    for future in hash_futures {
        match future.await {
            Ok(Ok((id, hash))) => {
                note_hashes.insert(id, hash);
            }
            Ok(Err(e)) => return Err(e),
            Err(_) => {
                return Err(NoteError::RequestError(
                    reqwest::Client::new()
                        .get("http://dummy")
                        .send()
                        .await
                        .unwrap_err(),
                ))
            }
        }
    }

    Ok(note_hashes)
}

async fn get_all_note_hashes(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteHash>>, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let all_notes =
        NoteWithParent::get_all(&mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let hash_map = compute_all_note_hashes(all_notes)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut note_hashes: Vec<_> = hash_map
        .into_iter()
        .map(|(id, hash)| NoteHash { id, hash })
        .collect();
    note_hashes.sort_by_key(|h| h.id);

    Ok(Json(note_hashes))
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
    use crate::schema::note_hierarchy::dsl::note_hierarchy;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all notes
    let all_notes =
        NoteWithoutFts::get_all(&mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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
        notes_map: &HashMap<i32, &NoteWithoutFts>,
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
            title: Some(note.title.clone()),
            content: Some(note.content.clone()),
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
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = NoteHierarchy::get_hierarchy_mappings(&mut conn).map_err(|e| {
        tracing::error!("Error getting hierarchy mappings: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(response))
}

async fn create_note(
    State(state): State<AppState>,
    Json(payload): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteWithoutFts>), StatusCode> {
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

// Handler for the PUT /notes/tree endpoint
pub async fn update_note_tree(
    State(state): State<AppState>,
    Json(note_tree): Json<NoteTreeNode>,
) -> Result<StatusCode, StatusCode> {
    update_database_from_notetreenode(State(state), Json(note_tree)).await
}

async fn update_database_from_notetreenode(
    State(state): State<AppState>,
    Json(note_tree_node): Json<NoteTreeNode>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state.pool.get().map_err(|e| {
        eprintln!("Failed to get connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Recursive function to process each node
    fn process_node(
        conn: &mut PgConnection,
        mut node: NoteTreeNode,
        parent_id: Option<i32>,
    ) -> Result<i32, DieselError> {
        eprintln!("Processing node: id={}, title={:?}", node.id, node.title);
        use crate::schema::note_hierarchy::dsl::{child_note_id, note_hierarchy};
        use crate::schema::notes::dsl::{content, id as notes_id, modified_at, notes, title};
        // Determine if the note is new or existing
        let node_id = if node.id <= 0 {
            // Insert new note
            let new_note = NewNote {
                title: &node.title.unwrap_or_default(),
                content: &node.content.unwrap_or_default(),
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };
            let result = diesel::insert_into(notes)
                .values(&new_note)
                .returning(notes_id)
                .get_result::<i32>(conn);

            match result {
                Ok(id) => {
                    eprintln!("Inserted new note with id: {}", id);
                    id
                }
                Err(e) => {
                    eprintln!("Failed to insert new note: {:?}", e);
                    return Err(e);
                }
            }
        } else {
            // Update existing note
            diesel::update(notes.filter(notes_id.eq(node.id)))
                .set((
                    title.eq(&node.title.unwrap_or_default()),
                    content.eq(&node.content.unwrap_or_default()),
                    modified_at.eq(Some(chrono::Utc::now().naive_utc())),
                ))
                .execute(conn)?;
            node.id
        };

        // After determining 'node_id', but before deleting the existing hierarchy
        // NOTE this is because hierarchy_type is still not a core component.
        if node.hierarchy_type.is_none() {
            use crate::schema::note_hierarchy::dsl::*;

            // Retrieve the existing hierarchy_type from the database
            let existing_hierarchy = note_hierarchy
                .filter(child_note_id.eq(node_id))
                .first::<NoteHierarchy>(conn)
                .optional()?;

            if let Some(existing_h) = existing_hierarchy {
                // Assign the existing hierarchy_type to the node
                node.hierarchy_type = existing_h.hierarchy_type.clone();
            }
        }

        // Update hierarchy

        // Update hierarchy only if there is a parent
        if let Some(p_id) = parent_id {
            // Remove existing hierarchy entry for this node
            diesel::delete(note_hierarchy.filter(child_note_id.eq(node_id))).execute(conn)?;

            // Insert new hierarchy entry
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

// Single note rendering handlers
async fn render_note_html(
    Path(note_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<String, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let note = notes
        .find(note_id)
        .first::<Note>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(draftsmith_render::parse_md_to_html(&note.content))
}

async fn render_note_md(
    Path(note_id): Path<i32>,
    State(state): State<AppState>,
) -> Result<String, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let note = notes
        .find(note_id)
        .first::<Note>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(draftsmith_render::process_md(&note.content))
}

// All notes rendering handlers
async fn render_all_notes_html(
    State(state): State<AppState>,
) -> Result<Json<Vec<RenderedNote>>, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let notes =
        NoteWithoutFts::get_all(&mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rendered: Vec<RenderedNote> = notes
        .iter()
        .map(|note| RenderedNote {
            id: note.id,
            rendered_content: format!(
                "# {}\n\n{}",
                note.title,
                draftsmith_render::parse_md_to_html(&note.content)
            ),
        })
        .collect();

    Ok(Json(rendered))
}

async fn render_all_notes_md(
    State(state): State<AppState>,
) -> Result<Json<Vec<RenderedNote>>, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let notes =
        NoteWithoutFts::get_all(&mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rendered: Vec<RenderedNote> = notes
        .iter()
        .map(|note| RenderedNote {
            id: note.id,
            rendered_content: format!(
                "# {}\n\n{}",
                note.title,
                draftsmith_render::process_md(&note.content)
            ),
        })
        .collect();

    Ok(Json(rendered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::Json;
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

    struct TestCleanup {
        pool: Pool<ConnectionManager<PgConnection>>,
        note_ids: Vec<i32>,
    }

    impl Drop for TestCleanup {
        fn drop(&mut self) {
            if let Ok(mut conn) = self.pool.get() {
                use crate::schema::note_hierarchy::dsl::{
                    child_note_id, note_hierarchy, parent_note_id,
                };
                use crate::schema::notes::dsl::{id as notes_id, notes};

                // Clean up hierarchies first due to foreign key constraints
                let _ = diesel::delete(note_hierarchy)
                    .filter(
                        child_note_id
                            .eq_any(&self.note_ids)
                            .or(parent_note_id.eq_any(&self.note_ids)),
                    )
                    .execute(&mut conn);

                // Then clean up the notes
                let _ = diesel::delete(notes)
                    .filter(notes_id.eq_any(&self.note_ids))
                    .execute(&mut conn);
            }
        }
    }

    #[tokio::test]
    /// Tests the function to update notes from a supplied tree structure
    /// This can't use a conn.test_transaction block because
    /// the tree function is recursive and passing in a connection
    /// will add too much complexity to the test.
    /// This function automatically cleans up after itself via Drop trait.
    async fn test_update_database_from_notetreenode() {
        // Set up the test state
        let state = setup_test_state();
        let pool = state.pool.as_ref().clone();

        // Get unique content identifiers using timestamp
        let now = format!("{}", chrono::Utc::now());
        let root_content = format!("root_content_{}", now);
        let child1_content = format!("child1_content_{}", now);
        let child2_content = format!("child2_content_{}", now);

        // Create an input NoteTreeNode with new notes
        let input_tree = NoteTreeNode {
            id: 0,                       // Indicates a new note
            title: Some("".to_string()), // Title is read-only
            content: Some(root_content.clone()),
            created_at: None,
            modified_at: None,
            hierarchy_type: None,
            children: vec![
                NoteTreeNode {
                    id: 0,
                    title: Some("".to_string()),
                    content: Some(child1_content.clone()),
                    created_at: None,
                    modified_at: None,
                    hierarchy_type: Some("block".to_string()),
                    children: vec![],
                },
                NoteTreeNode {
                    id: 0,
                    title: Some("".to_string()),
                    content: Some(child2_content.clone()),
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

        // Assert that the operation was successful
        assert_eq!(
            response.expect("Update failed"),
            StatusCode::OK,
            "Expected status code OK"
        );

        // Obtain a connection from the pool
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get a connection from the pool");

        conn.test_transaction::<_, DieselError, _>(|conn| {
            // Check that the notes have been added
            use crate::schema::notes::dsl::*;
            let notes_in_db = notes
                .filter(content.eq_any(vec![
                    root_content.clone(),
                    child1_content.clone(),
                    child2_content.clone(),
                ]))
                .load::<Note>(conn)
                .expect("Failed to load notes from database");

            assert_eq!(
                notes_in_db.len(),
                3,
                "Expected 3 matching notes in the database"
            );

            // Create cleanup struct that will automatically clean up when dropped
            let _cleanup = TestCleanup {
                pool: pool.clone(),
                note_ids: notes_in_db.iter().map(|note| note.id).collect(),
            };

            // Find the notes by content
            let note_root = notes_in_db
                .iter()
                .find(|note| note.content == root_content)
                .expect("Root note not found");
            let note_child_1 = notes_in_db
                .iter()
                .find(|note| note.content == child1_content)
                .expect("Child note 1 not found");
            let note_child_2 = notes_in_db
                .iter()
                .find(|note| note.content == child2_content)
                .expect("Child note 2 not found");

            // Verify hierarchy
            use crate::schema::note_hierarchy::dsl::*;
            let hierarchies_in_db = note_hierarchy
                .filter(child_note_id.eq_any(vec![note_child_1.id, note_child_2.id]))
                .load::<NoteHierarchy>(conn)
                .expect("Failed to load hierarchy from database");

            assert_eq!(
                hierarchies_in_db.len(),
                2,
                "Expected 2 hierarchy entries in the database"
            );

            // Verify parent IDs
            for hierarchy in hierarchies_in_db {
                assert_eq!(
                    hierarchy.parent_note_id,
                    Some(note_root.id),
                    "Hierarchy parent ID does not match root note ID"
                );
            }

            Ok(())
        })
    }

    #[tokio::test]
    async fn test_batch_update_notes() {
        let state = setup_test_state();
        let pool = state.pool.as_ref().clone();
        let mut conn = pool.get().expect("Failed to get connection");

        // Create test notes
        let now = format!("{}", chrono::Utc::now());
        let note1_title = format!("test_note1_{}", now);
        let note2_title = format!("test_note2_{}", now);

        let note1 = diesel::insert_into(crate::schema::notes::table)
            .values(NewNote {
                title: &note1_title,
                content: "original content 1",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create note 1");

        let note2 = diesel::insert_into(crate::schema::notes::table)
            .values(NewNote {
                title: &note2_title,
                content: "original content 2",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create note 2");

        let _cleanup = TestCleanup {
            pool: pool.clone(),
            note_ids: vec![note1.id, note2.id],
        };

        // Create batch update request
        let updates = vec![
            (
                note1.id,
                UpdateNoteRequest {
                    title: Some("Updated Title 1".to_string()),
                    content: "Updated Content 1".to_string(),
                },
            ),
            (
                note2.id,
                UpdateNoteRequest {
                    title: Some("Updated Title 2".to_string()),
                    content: "Updated Content 2".to_string(),
                },
            ),
        ];

        let batch_request = BatchUpdateRequest { updates };

        // Perform batch update
        let response = update_notes(State(state), Json(batch_request))
            .await
            .expect("Failed to perform batch update");

        let batch_response = response.0;
        assert_eq!(
            batch_response.updated.len(),
            2,
            "Expected 2 successful updates"
        );
        assert_eq!(batch_response.failed.len(), 0, "Expected no failed updates");

        // Verify updates in database
        use crate::schema::notes::dsl::*;
        let updated_notes = notes
            .filter(id.eq_any(vec![note1.id, note2.id]))
            .load::<Note>(&mut conn)
            .expect("Failed to load updated notes");

        assert_eq!(updated_notes.len(), 2, "Expected 2 notes in database");

        let updated_note1 = updated_notes.iter().find(|n| n.id == note1.id).unwrap();
        let updated_note2 = updated_notes.iter().find(|n| n.id == note2.id).unwrap();

        // Title is automatically set as H1 of content by Database
        // See commit 12acc9fb1b177b279181c4d15618e60571722ca1
        // assert_eq!(updated_note1.title, "Updated Title 1");
        assert_eq!(updated_note1.content, "Updated Content 1");
        // assert_eq!(updated_note2.title, "Updated Title 2");
        assert_eq!(updated_note2.content, "Updated Content 2");
    }

    #[tokio::test]
    async fn test_batch_update_notes_client() {
        let state = setup_test_state();

        // Create test notes
        let note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Test Note 1".to_string(),
                content: "Original content 1".to_string(),
            }),
        )
        .await
        .expect("Failed to create note 1")
        .1
         .0;

        let note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Test Note 2".to_string(),
                content: "Original content 2".to_string(),
            }),
        )
        .await
        .expect("Failed to create note 2")
        .1
         .0;

        // Prepare batch updates
        let updates = vec![
            (
                note1.id,
                UpdateNoteRequest {
                    title: Some("Updated Note 1".to_string()),
                    content: "Updated content 1".to_string(),
                },
            ),
            (
                note2.id,
                UpdateNoteRequest {
                    title: Some("Updated Note 2".to_string()),
                    content: "Updated content 2".to_string(),
                },
            ),
        ];

        // Perform batch update
        let result = update_notes(State(state.clone()), Json(BatchUpdateRequest { updates }))
            .await
            .unwrap()
            .0;

        // Verify results
        assert_eq!(result.updated.len(), 2, "Expected 2 successful updates");
        assert_eq!(result.failed.len(), 0, "Expected no failed updates");

        // Verify the updates by fetching the notes
        let updated_note1 = get_note(Path(note1.id), State(state.clone()))
            .await
            .unwrap()
            .0;

        let updated_note2 = get_note(Path(note2.id), State(state.clone()))
            .await
            .unwrap()
            .0;

        assert_eq!(updated_note1.content, "Updated content 1");
        assert_eq!(updated_note2.content, "Updated content 2");

        // Clean up
        let _ = delete_note(Path(note1.id), State(state.clone())).await;
        let _ = delete_note(Path(note2.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_update_existing_note_hierarchy() {
        // Set up the test state
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get a connection from the pool");

        // Get posix timestamp for unique titles
        let now = format!("{}", chrono::Utc::now());
        let root_title = format!("test_existing_root_{}", now);
        let child1_title = format!("test_existing_child1_{}", now);
        let child2_title = format!("test_existing_child2_{}", now);

        // Note Content
        let note_root_content_original = "root content";
        let note_root_content_updated = "updated root content";
        let note_1_content_original = "Original content for child1";
        let note_2_content_original = "Original content for child2";
        let note_1_content_updated = "Updated content for child1";
        let note_2_content_updated = "Updated content for child2";

        // Create three notes
        use crate::schema::notes::dsl::*;
        let root_note = diesel::insert_into(notes)
            .values(NewNote {
                title: &root_title,
                content: note_root_content_original,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create root note");

        let child1_note = diesel::insert_into(notes)
            .values(NewNote {
                title: &child1_title,
                content: note_1_content_original,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create child1 note");

        let child2_note = diesel::insert_into(notes)
            .values(NewNote {
                title: &child2_title,
                content: note_2_content_original,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create child2 note");

        // Create initial hierarchy: root -> child1 -> child2
        use crate::schema::note_hierarchy::dsl::*;
        diesel::insert_into(note_hierarchy)
            .values(&NewNoteHierarchy {
                child_note_id: Some(child1_note.id),
                parent_note_id: Some(root_note.id),
                hierarchy_type: Some("block"),
            })
            .execute(&mut conn)
            .expect("Failed to create first hierarchy link");

        diesel::insert_into(note_hierarchy)
            .values(&NewNoteHierarchy {
                child_note_id: Some(child2_note.id),
                parent_note_id: Some(child1_note.id),
                hierarchy_type: Some("block"),
            })
            .execute(&mut conn)
            .expect("Failed to create second hierarchy link");

        let root_id = root_note.id;
        let child1_id = child1_note.id;
        let child2_id = child2_note.id;

        // Create cleanup struct that will automatically clean up when dropped
        let _cleanup = TestCleanup {
            pool: state.pool.as_ref().clone(),
            note_ids: vec![root_id, child1_id, child2_id],
        };

        // Create a new tree structure where child2 is directly under root, and child1 is under child2
        let modified_tree = NoteTreeNode {
            id: root_id,
            title: Some(root_title),
            content: Some(note_root_content_updated.to_string()),
            created_at: None,
            modified_at: None,
            hierarchy_type: None,
            children: vec![NoteTreeNode {
                id: child2_id,
                title: Some(child2_title),
                content: Some(note_2_content_updated.to_string()),
                created_at: None,
                modified_at: None,
                hierarchy_type: Some("block".to_string()),
                children: vec![NoteTreeNode {
                    id: child1_id,
                    title: Some(child1_title),
                    content: Some(note_1_content_updated.to_string()),
                    created_at: None,
                    modified_at: None,
                    hierarchy_type: Some("block".to_string()),
                    children: vec![],
                }],
            }],
        };

        // Update the hierarchy
        let response = update_database_from_notetreenode(State(state.clone()), Json(modified_tree))
            .await
            .expect("Failed to update hierarchy");
        assert_eq!(response, StatusCode::OK);

        // Verify the new hierarchy structure
        // Verify the new hierarchy structure
        // Check child2 is now directly under root
        let root_children = note_hierarchy
            .filter(parent_note_id.eq(root_id))
            .load::<NoteHierarchy>(&mut conn)
            .expect("Failed to load root children");
        assert_eq!(root_children.len(), 1);
        assert_eq!(root_children[0].child_note_id, Some(child2_id));
        assert_eq!(root_children[0].hierarchy_type, Some("block".to_string()));

        // Check child1 is now under child2
        let child2_children = note_hierarchy
            .filter(parent_note_id.eq(child2_id))
            .load::<NoteHierarchy>(&mut conn)
            .expect("Failed to load child2 children");
        assert_eq!(child2_children.len(), 1);
        assert_eq!(child2_children[0].child_note_id, Some(child1_id));
        assert_eq!(child2_children[0].hierarchy_type, Some("block".to_string()));

        // Check child1 has no children
        let child1_children = note_hierarchy
            .filter(parent_note_id.eq(child1_id))
            .load::<NoteHierarchy>(&mut conn)
            .expect("Failed to load child1 children");
        assert_eq!(child1_children.len(), 0);

        // check that the note content has been updated
        use crate::schema::notes::dsl::id as notes_id;
        let updated_notes = notes
            .filter(notes_id.eq_any(vec![root_id, child1_id, child2_id]))
            .load::<Note>(&mut conn)
            .expect("Failed to load notes from database");

        assert_eq!(updated_notes.len(), 3);

        let updated_root = updated_notes
            .iter()
            .find(|note| note.id == root_id)
            .expect("Root note not found");
        let updated_child1 = updated_notes
            .iter()
            .find(|note| note.id == child1_id)
            .expect("Child note 1 not found");
        let updated_child2 = updated_notes
            .iter()
            .find(|note| note.id == child2_id)
            .expect("Child note 2 not found");

        assert_eq!(updated_root.content, note_root_content_updated);
        assert_eq!(updated_child1.content, note_1_content_updated);
        assert_eq!(updated_child2.content, note_2_content_updated);
    }
    #[tokio::test]
    async fn test_get_all_note_hashes() {
        let state = setup_test_state();
        let pool = state.pool.as_ref().clone();
        let mut conn = pool.get().expect("Failed to get connection");

        // Create test notes with unique titles using timestamp
        let now = format!("{}", chrono::Utc::now());
        let note1_title = format!("test_note1_{}", now);
        let note2_title = format!("test_note2_{}", now);

        // Create two test notes
        let note1 = diesel::insert_into(crate::schema::notes::table)
            .values(NewNote {
                title: &note1_title,
                content: "test content 1",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create note 1");

        let note2 = diesel::insert_into(crate::schema::notes::table)
            .values(NewNote {
                title: &note2_title,
                content: "test content 2",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create note 2");

        let _cleanup = TestCleanup {
            pool: pool.clone(),
            note_ids: vec![note1.id, note2.id],
        };

        // Get hashes for all notes
        let response = get_all_note_hashes(State(state))
            .await
            .expect("Failed to get note hashes");

        let note_hashes = response.0;

        // Verify both notes are present
        assert!(note_hashes.iter().any(|nh| nh.id == note1.id));
        assert!(note_hashes.iter().any(|nh| nh.id == note2.id));

        // Get NoteWithParent instances for hash verification
        let note1_with_parent = NoteWithParent::get_by_id(&mut conn, note1.id)
            .expect("Failed to get note1 with parent");
        let note2_with_parent = NoteWithParent::get_by_id(&mut conn, note2.id)
            .expect("Failed to get note2 with parent");

        let note1_hash = note_hashes.iter().find(|nh| nh.id == note1.id).unwrap();
        let note2_hash = note_hashes.iter().find(|nh| nh.id == note2.id).unwrap();

        assert_eq!(note1_hash.hash, compute_note_hash(&note1_with_parent));
        assert_eq!(note2_hash.hash, compute_note_hash(&note2_with_parent));
    }

    #[tokio::test]
    async fn test_note_rendering() {
        use crate::schema::notes::dsl::*;

        let state = setup_test_state();
        let pool = state.pool.as_ref().clone();
        let mut conn = pool.get().expect("Failed to get connection");

        // Create test notes with markdown content
        let test_content1 = "# Test Header 1\n\nThis is a **test** note with _markdown_.";
        let test_content2 = "# Test Header 2\n\nThis is another **test** note with _markdown_.";

        let note1 = diesel::insert_into(notes)
            .values(NewNote {
                title: "Test Note 1",
                content: test_content1,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create test note 1");

        let note2 = diesel::insert_into(notes)
            .values(NewNote {
                title: "Test Note 2",
                content: test_content2,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<Note>(&mut conn)
            .expect("Failed to create test note 2");

        let _cleanup = TestCleanup {
            pool: pool.clone(),
            note_ids: vec![note1.id, note2.id],
        };

        // Test single note HTML rendering
        let html_response = render_note_html(Path(note1.id), State(state.clone()))
            .await
            .expect("Failed to render HTML");
        assert!(html_response.contains("<h1>"));
        assert!(html_response.contains("<strong>test</strong>"));
        assert!(html_response.contains("<em>markdown</em>"));

        // Test single note MD rendering
        let md_response = render_note_md(Path(note1.id), State(state.clone()))
            .await
            .expect("Failed to render MD");
        assert!(md_response.contains("# Test Header"));
        assert!(md_response.contains("**test**"));
        assert!(md_response.contains("_markdown_"));

        // Test all notes HTML rendering
        let all_html_response = render_all_notes_html(State(state.clone()))
            .await
            .expect("Failed to render all HTML");
        let rendered_notes_html = all_html_response.0;

        assert!(rendered_notes_html.len() >= 2);
        let note1_html = rendered_notes_html
            .iter()
            .find(|n| n.id == note1.id)
            .unwrap();
        assert!(note1_html.rendered_content.contains("<h1>"));
        assert!(note1_html
            .rendered_content
            .contains("<strong>test</strong>"));

        // Test all notes MD rendering
        let all_md_response = render_all_notes_md(State(state.clone()))
            .await
            .expect("Failed to render all MD");
        let rendered_notes_md = all_md_response.0;

        assert!(rendered_notes_md.len() >= 2);
        let note1_md = rendered_notes_md.iter().find(|n| n.id == note1.id).unwrap();
        assert!(note1_md.rendered_content.contains("# Test Header"));
        assert!(note1_md.rendered_content.contains("**test**"));
    }
}
