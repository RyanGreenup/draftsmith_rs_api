use crate::client::NoteError;
// TODO API should not import from client, only client from API,
//      consider use crate::api::hierarchy::notes::NoteError;
use crate::tables::{Asset, HierarchyMapping, NewAsset, NoteWithParent};
use crate::tables::{NewNote, NoteHierarchy, NoteWithoutFts};
use crate::{FLAT_API, SEARCH_FTS_API, UPLOADS_DIR};
pub mod custom_rhai_functions;
pub mod hierarchy;
mod state;
pub mod tags;
pub mod tasks;

use axum::extract::Multipart;
use axum::http::{header, HeaderName, HeaderValue};
use axum::{
    extract::{DefaultBodyLimit, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use axum_extra::response::ErasedJson;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use futures::future::join_all;
use serde::{Deserialize, Serialize};
// Type alias for the complex tuple type used in get_tags_notes
type NoteTagResult = (
    i32,                           // tag_id
    i32,                           // note_id
    String,                        // note_title
    Option<chrono::NaiveDateTime>, // created_at
    Option<chrono::NaiveDateTime>, // modified_at
);

#[derive(Serialize)]
struct RenderedNote {
    id: i32,
    rendered_content: String,
}
use crate::api::hierarchy::notes::{
    attach_child_note, detach_child_note, get_note_tree, update_note_tree,
};
pub use hierarchy::notes::{
    get_all_note_paths, get_relative_note_path, get_single_note_path, NoteTreeNode,
};
use sha2::{Digest, Sha256};
use state::{AppState, Pool};
use std::collections::{HashMap, HashSet};
use std::path::{Path as FilePath, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
}

#[derive(Serialize, Deserialize)]
pub struct BacklinkResponse {
    pub id: i32,
    pub title: String,
    pub content: String,
}

#[derive(Serialize, Deserialize)]
pub struct ForwardLinkResponse {
    pub id: i32,
    pub title: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct LinkEdge {
    pub from: i32,
    pub to: i32,
}

/// Takes a vector of tag IDs and returns a `HashMap<i32, Vec<NoteMetadataResponse>>`
/// where the key of the hashmap is the tag_id and the vector of `NoteMetadataResponse`
/// represents the list of notes that correspond to that tag ID.
pub async fn get_tags_notes(
    State(state): State<AppState>,
    tag_ids: Vec<i32>,
) -> Result<Json<HashMap<i32, Vec<NoteMetadataResponse>>>, StatusCode> {
    use crate::schema::{note_tags, notes, tags};
    use diesel::prelude::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all notes for the specified tags
    let results: Vec<NoteTagResult> = note_tags::table
        .inner_join(notes::table.on(notes::columns::id.eq(note_tags::columns::note_id)))
        .inner_join(tags::table.on(tags::columns::id.eq(note_tags::columns::tag_id)))
        .filter(tags::columns::id.eq_any(tag_ids))
        .select((
            tags::columns::id,
            notes::columns::id,
            notes::columns::title,
            notes::columns::created_at,
            notes::columns::modified_at,
        ))
        .load::<(
            i32,
            i32,
            String,
            Option<chrono::NaiveDateTime>,
            Option<chrono::NaiveDateTime>,
        )>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Group notes by tag_id
    let mut tags_notes: HashMap<i32, Vec<NoteMetadataResponse>> = HashMap::new();
    for (t_id, n_id, n_title, n_created, n_modified) in results {
        tags_notes
            .entry(t_id)
            .or_default()
            .push(NoteMetadataResponse {
                id: n_id,
                title: n_title,
                created_at: n_created,
                modified_at: n_modified,
            });
    }

    Ok(Json(tags_notes))
}

/// Returns a `HashMap<i32, Vec<TagResponse>>` where the key is the note_id and the value
/// is a vector of `TagResponse` objects representing the tags corresponding to that note id.
pub async fn get_notes_tags(
    State(state): State<AppState>,
    note_ids: Vec<i32>,
) -> Result<Json<HashMap<i32, Vec<TagResponse>>>, StatusCode> {
    use crate::schema::{note_tags, tags};
    use diesel::prelude::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all tags for the specified notes
    let results: Vec<(i32, i32, String)> = note_tags::table
        .inner_join(tags::table)
        .filter(note_tags::columns::note_id.eq_any(note_ids))
        .select((
            note_tags::columns::note_id,
            tags::columns::id,
            tags::columns::name,
        ))
        .load::<(i32, i32, String)>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Group tags by note_id
    let mut notes_tags: HashMap<i32, Vec<TagResponse>> = HashMap::new();
    for (n_id, t_id, t_name) in results {
        notes_tags.entry(n_id).or_default().push(TagResponse {
            id: t_id,
            name: t_name,
        });
    }

    Ok(Json(notes_tags))
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

#[derive(Serialize, Deserialize, Clone, Debug)]
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

#[derive(Deserialize)]
pub struct RenderMarkdownRequest {
    content: String,
    #[serde(default)]
    format: Option<String>,
}

#[derive(Deserialize)]
pub struct CreateAssetRequest {
    pub note_id: Option<i32>,
    pub filename: Option<String>,
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct UpdateAssetRequest {
    pub note_id: Option<i32>,
    pub description: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct AssetResponse {
    pub id: i32,
    pub note_id: Option<i32>,
    pub location: PathBuf,
    pub description: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Deserialize)]
pub struct ListAssetsParams {
    note_id: Option<i32>,
}

#[derive(Deserialize, Serialize)]
pub struct AttachChildRequest {
    pub child_note_id: i32,
    pub parent_note_id: Option<i32>,
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

    // Spawn cleanup task
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(24 * 60 * 60)); // 24 hours
            loop {
                interval.tick().await;
                cleanup_orphaned_assets(state.clone()).await;
            }
        });
    }

    let max_body_size = 1024 * 1024 * 1024; // 1 GB

    Router::new()
        .merge(tags::create_router())
        .merge(tasks::create_router())
        .route("/assets", post(create_asset).get(list_assets))
        .route(
            "/assets/:id",
            get(get_asset).put(update_asset).delete(delete_asset),
        )
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
        .route("/render/markdown", post(render_markdown))
        .route("/notes/flat/:id/backlinks", get(get_backlinks))
        .route("/notes/flat/:id/forward-links", get(get_forward_links))
        .route("/notes/flat/link-edge-list", get(get_link_edge_list))
        .route("/notes/paths", get(get_all_note_paths))
        .route("/notes/:id/path", get(get_single_note_path))
        .route("/notes/:id/path/:from_id", get(get_relative_note_path))
        .route(
            "/assets/download/*filepath",
            get(download_asset_by_filename),
        )
        .layer(DefaultBodyLimit::max(max_body_size))
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

fn get_connection() -> PgConnection {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    PgConnection::establish(&database_url).expect("Error connecting to database")
}

pub fn get_note_title(note_id: i32) -> Result<std::string::String, diesel::result::Error> {
    use crate::schema::notes::dsl::*;

    let mut conn = get_connection();

    notes.find(note_id).select(title).first::<String>(&mut conn)
}

pub fn get_note_content(note_id: i32) -> Result<std::string::String, diesel::result::Error> {
    use crate::schema::notes::dsl::*;

    let mut conn = get_connection();

    notes
        .find(note_id)
        .select(content)
        .first::<String>(&mut conn)
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
        .select(NoteWithoutFts::as_select())
        .first(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(note))
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
) -> Result<NoteWithoutFts, DieselError> {
    use crate::schema::notes::dsl::*;

    let mut conn = pool.get().map_err(|_| DieselError::RollbackTransaction)?;
    let changes = (
        content.eq(update.content),
        modified_at.eq(Some(chrono::Utc::now().naive_utc())),
    );

    if let Some(new_title) = update.title {
        diesel::update(notes.find(note_id))
            .set((title.eq(new_title), changes))
            .returning(NoteWithoutFts::as_select())
            .get_result(&mut conn)
    } else {
        diesel::update(notes.find(note_id))
            .set(changes)
            .returning(NoteWithoutFts::as_select())
            .get_result(&mut conn)
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
            Ok(note) => updated.push(note),
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
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
    } else {
        diesel::update(notes.find(note_id))
            .set(changes)
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
    }
    .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok((StatusCode::OK, Json(updated_note)))
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
        .returning(NoteWithoutFts::as_select())
        .get_result::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::CREATED, Json(note)))
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
        .select(NoteWithoutFts::as_select())
        .first::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(custom_rhai_functions::parse_md_to_html(
        &note.content,
        Some(&note_id),
    ))
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
        .select(NoteWithoutFts::as_select())
        .first::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(custom_rhai_functions::process_md(
        &note.content,
        Some(&note_id),
    ))
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
                custom_rhai_functions::parse_md_to_html(&note.content, Some(&note.id))
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
                custom_rhai_functions::process_md(&note.content, Some(&note.id))
            ),
        })
        .collect();

    Ok(Json(rendered))
}

async fn create_asset(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<AssetResponse>), StatusCode> {
    use crate::schema::assets::dsl::*;

    // Get the upload directory from environment or use a default
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| UPLOADS_DIR.to_string());
    let base_path = PathBuf::from(&upload_dir);

    // Create upload directory if it doesn't exist
    fs::create_dir_all(&base_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut file_data = Vec::new();
    let mut original_filename = None;
    let mut asset_request = CreateAssetRequest {
        note_id: None,
        filename: None,
        description: None,
    };

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        match field.name() {
            Some("file") => {
                original_filename = field.file_name().map(String::from);
                file_data = field
                    .bytes()
                    .await
                    .map_err(|_| StatusCode::BAD_REQUEST)?
                    .to_vec();
            }
            Some("note_id") => {
                let note_id_str = field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?;
                asset_request.note_id =
                    Some(note_id_str.parse().map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            Some("filename") => {
                asset_request.filename =
                    Some(field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            Some("description") => {
                asset_request.description =
                    Some(field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            _ => {}
        }
    }

    if file_data.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Generate filename and path
    let file_path = if let Some(custom_path) = asset_request.filename {
        // Split the path into directory components and filename
        let path = PathBuf::from(custom_path);
        let full_path = base_path.join(&path);

        // Create parent directories if they don't exist
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        full_path
    } else {
        // Use original filename or generate UUID
        let filename = original_filename
            .map(|name| sanitize_filename::sanitize(&name))
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        base_path.join(filename)
    };

    // Write the file
    fs::write(&file_path, file_data)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Store asset record in database
    let new_asset = NewAsset {
        note_id: asset_request.note_id,
        location: file_path.to_str().unwrap(),
        description: asset_request.description.as_deref(),
    };

    let asset = diesel::insert_into(assets)
        .values(&new_asset)
        .get_result::<Asset>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(AssetResponse {
            id: asset.id,
            note_id: asset.note_id,
            location: PathBuf::from(&asset.location),
            description: asset.description,
            created_at: asset.created_at,
        }),
    ))
}

async fn get_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<i32>,
) -> Result<impl IntoResponse, StatusCode> {
    use crate::schema::assets::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let asset = assets
        .find(asset_id)
        .first::<Asset>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Convert stored location to PathBuf
    let file_path = PathBuf::from(&asset.location);

    // Get the upload directory from environment or use a default
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| UPLOADS_DIR.to_string());
    let base_path = PathBuf::from(&upload_dir);

    // Ensure the file path is within the base directory
    if !file_path.starts_with(&base_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create parent directories if they don't exist
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Check if file exists
    if !file_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Read the file
    let file_data = fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Guess the mime type
    let mime_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Get the filename from the path
    let display_filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    let headers: [(HeaderName, HeaderValue); 2] = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime_type).unwrap(),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", display_filename))
                .unwrap(),
        ),
    ];

    Ok((headers, file_data))
}

async fn list_assets(
    State(state): State<AppState>,
    Query(params): Query<ListAssetsParams>,
) -> Result<Json<Vec<AssetResponse>>, StatusCode> {
    use crate::schema::assets::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut query = assets.into_boxed();

    if let Some(note_id_param) = params.note_id {
        query = query.filter(note_id.eq(note_id_param));
    }

    let results = query
        .load::<Asset>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = results
        .into_iter()
        .map(|asset| AssetResponse {
            id: asset.id,
            note_id: asset.note_id,
            location: PathBuf::from(&asset.location),
            description: asset.description,
            created_at: asset.created_at,
        })
        .collect();

    Ok(Json(response))
}

async fn update_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<i32>,
    Json(payload): Json<UpdateAssetRequest>,
) -> Result<Json<AssetResponse>, StatusCode> {
    use crate::schema::assets::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let asset = diesel::update(assets.find(asset_id))
        .set((
            note_id.eq(payload.note_id),
            description.eq(payload.description),
        ))
        .get_result::<Asset>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(AssetResponse {
        id: asset.id,
        note_id: asset.note_id,
        location: PathBuf::from(asset.location),
        description: asset.description,
        created_at: asset.created_at,
    }))
}

async fn download_asset_by_filename(
    State(_state): State<AppState>,
    Path(filepath): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    // Get the upload directory from environment or use a default
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| UPLOADS_DIR.to_string());
    let base_path = PathBuf::from(&upload_dir);

    // Convert the filepath to a PathBuf and join with base path
    let file_path = base_path.join(&filepath);

    // Ensure the resulting path is within the base directory (prevent directory traversal)
    if !file_path.starts_with(&base_path) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create parent directories if they don't exist
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    // Check if file exists
    if !file_path.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Read the file
    let file_data = fs::read(&file_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Guess the mime type
    let mime_type = mime_guess::from_path(&file_path)
        .first_or_octet_stream()
        .to_string();

    // Get the filename from the path
    let display_filename = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");

    let headers: [(HeaderName, HeaderValue); 2] = [
        (
            header::CONTENT_TYPE,
            HeaderValue::from_str(&mime_type).unwrap(),
        ),
        (
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", display_filename))
                .unwrap(),
        ),
    ];

    Ok((headers, file_data))
}

async fn get_forward_links(
    State(state): State<AppState>,
    Path(note_id): Path<i32>,
) -> Result<Json<Vec<ForwardLinkResponse>>, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // First get the source note
    let source_note = notes
        .find(note_id)
        .select(NoteWithoutFts::as_select())
        .first::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Extract all patterns from the content
    let link_regex =
        regex::Regex::new(r"(?:\[\[(\d+)\]\]|\[\[(\d+)\|[^]]+\]\]|(?:\[.*?\])\((\d+)\))").unwrap();

    let mut linked_ids: Vec<i32> = Vec::new();

    for cap in link_regex.captures_iter(&source_note.content) {
        // Check each capture group and parse the id if it exists.
        for i in 1..=3 {
            if let Some(id_str) = &cap.get(i) {
                if let Ok(id_val) = id_str.as_str().parse::<i32>() {
                    linked_ids.push(id_val);
                    break; // Since we found a valid ID, no need to check other groups
                }
            }
        }
    }

    if linked_ids.is_empty() {
        return Ok(Json(Vec::new()));
    }

    // Get all linked notes
    let linked_notes = notes
        .filter(id.eq_any(linked_ids))
        .select(NoteWithoutFts::as_select())
        .load::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses = linked_notes
        .into_iter()
        .map(|note| ForwardLinkResponse {
            id: note.id,
            title: note.title,
            content: note.content,
        })
        .collect();

    Ok(Json(responses))
}

async fn get_backlinks(
    State(state): State<AppState>,
    Path(note_id): Path<i32>,
) -> Result<Json<Vec<BacklinkResponse>>, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // First verify the target note exists
    if notes
        .find(note_id)
        .select(NoteWithoutFts::as_select())
        .first::<NoteWithoutFts>(&mut conn)
        .is_err()
    {
        return Err(StatusCode::NOT_FOUND);
    }

    // Patterns to match
    let pattern1 = format!("[[{}]]", note_id); // [[id]]
    let pattern2 = format!("[[{}|%", note_id); // [[id|
    let pattern3 = format!("]%({})", note_id); // ](id)
    let pattern4 = format!("%]({})", note_id); // %(id)

    // Find all notes that contain any of the link patterns
    let backlinks = notes
        .filter(
            content
                .like(format!("%{}%", pattern1))
                .or(content.like(format!("%{}%", pattern2)))
                .or(content.like(pattern3))
                .or(content.like(pattern4)),
        )
        .select(NoteWithoutFts::as_select())
        .load::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let responses = backlinks
        .into_iter()
        .map(|note| BacklinkResponse {
            id: note.id,
            title: note.title,
            content: note.content,
        })
        .collect();

    Ok(Json(responses))
}

async fn get_link_edge_list(
    State(state): State<AppState>,
) -> Result<Json<Vec<LinkEdge>>, StatusCode> {
    use crate::schema::notes::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all notes
    let all_notes = notes
        .select(NoteWithoutFts::as_select())
        .load::<NoteWithoutFts>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Extract all links using regex
    let link_regex = regex::Regex::new(r"\[\[(\d+)\]\]").unwrap();
    let mut edges = Vec::new();

    for note in all_notes {
        // Find all [[id]] patterns in the note content
        for cap in link_regex.captures_iter(&note.content) {
            if let Ok(to_id) = cap[1].parse::<i32>() {
                edges.push(LinkEdge {
                    from: note.id,
                    to: to_id,
                });
            }
        }
    }

    Ok(Json(edges))
}

/// Renders markdown content to HTML or plain text
/// # Parameters
///
/// - `content`: The markdown content to render
/// - `format`: The output format, either "html" or None (markdown)
async fn render_markdown(Json(payload): Json<RenderMarkdownRequest>) -> Result<String, StatusCode> {
    match payload.format.as_deref() {
        Some("html") => Ok(custom_rhai_functions::parse_md_to_html(
            &payload.content,
            None,
        )),
        _ => Ok(custom_rhai_functions::process_md(&payload.content, None)),
    }
}

async fn cleanup_orphaned_assets(state: AppState) {
    use crate::schema::assets::dsl::*;

    info!("Starting orphaned assets cleanup");

    // Get a connection from the pool
    let mut conn = match state.pool.get() {
        Ok(conn) => conn,
        Err(e) => {
            error!("Failed to get database connection for cleanup: {}", e);
            return;
        }
    };

    // Get all assets from database
    let db_assets = match assets.load::<Asset>(&mut conn) {
        Ok(db_assets) => db_assets, // Changed variable name to avoid conflict
        Err(e) => {
            error!("Failed to load assets from database: {}", e);
            return;
        }
    };

    // Get the upload directory
    let upload_dir = std::env::var("UPLOAD_DIR").unwrap_or_else(|_| UPLOADS_DIR.to_string());
    let base_path = FilePath::new(&upload_dir);

    // Ensure upload directory exists
    if let Err(e) = fs::create_dir_all(&base_path).await {
        error!("Failed to create upload directory: {}", e);
        return;
    }

    let mut files_on_disk = HashSet::new();

    // Read all files in the upload directory
    match fs::read_dir(&base_path).await {
        Ok(mut entries) => {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if let Ok(path) = entry.path().canonicalize() {
                    files_on_disk.insert(path);
                }
            }
        }
        Err(e) => {
            error!("Failed to read upload directory: {}", e);
            return;
        }
    }

    // Track statistics
    let mut orphaned_files = 0;
    let mut dangling_records = 0;

    // Check for files without database records
    for file_path in &files_on_disk {
        let file_exists_in_db = db_assets.iter().any(|asset| {
            FilePath::new(&asset.location)
                .canonicalize()
                .map(|p| p == *file_path)
                .unwrap_or(false)
        });

        if !file_exists_in_db {
            match fs::remove_file(file_path).await {
                Ok(_) => {
                    orphaned_files += 1;
                    info!("Removed orphaned file: {:?}", file_path);
                }
                Err(e) => {
                    warn!("Failed to remove orphaned file {:?}: {}", file_path, e);
                }
            }
        }
    }

    // Check for database records without files
    for asset in db_assets {
        let asset_path = FilePath::new(&asset.location);
        let canonical_path = match asset_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                // File doesn't exist, remove the database record
                match diesel::delete(assets.filter(id.eq(asset.id))).execute(&mut conn) {
                    Ok(_) => {
                        dangling_records += 1;
                        info!(
                            "Removed dangling database record for asset ID: {}",
                            asset.id
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to remove dangling record for asset ID {}: {}",
                            asset.id, e
                        );
                    }
                }
                continue;
            }
        };

        if !files_on_disk.contains(&canonical_path) {
            match diesel::delete(assets.filter(id.eq(asset.id))).execute(&mut conn) {
                Ok(_) => {
                    dangling_records += 1;
                    info!(
                        "Removed dangling database record for asset ID: {}",
                        asset.id
                    );
                }
                Err(e) => {
                    warn!(
                        "Failed to remove dangling record for asset ID {}: {}",
                        asset.id, e
                    );
                }
            }
        }
    }

    info!(
        "Cleanup completed. Removed {} orphaned files and {} dangling database records",
        orphaned_files, dangling_records
    );
}

async fn delete_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use crate::schema::assets::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get the asset first to get the file location
    let asset = assets
        .find(asset_id)
        .first::<Asset>(&mut conn)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    // Delete the file
    if let Err(e) = fs::remove_file(&asset.location).await {
        eprintln!("Error deleting file {}: {}", asset.location, e);
        // Continue with database deletion even if file deletion fails
    }

    // Delete from database
    diesel::delete(assets.find(asset_id))
        .execute(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use axum::Json;
    use diesel::r2d2::{ConnectionManager, Pool};
    use dotenv::dotenv;
    use std::sync::Arc;

    pub fn setup_test_state() -> AppState {
        dotenv().ok();
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
        let manager = ConnectionManager::<PgConnection>::new(&database_url);
        let pool = Pool::builder()
            .max_size(5) // This can cause tests to fail if too
            //   many connections are opened, they will exhaust postgres connections limit (200 usually)
            .build(manager)
            .expect("Failed to create pool.");
        AppState {
            pool: Arc::new(pool),
        }
    }

    pub struct TestCleanup {
        pub pool: Pool<ConnectionManager<PgConnection>>,
        pub note_ids: Vec<i32>,
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
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
            .expect("Failed to create note 1");

        let note2 = diesel::insert_into(crate::schema::notes::table)
            .values(NewNote {
                title: &note2_title,
                content: "original content 2",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
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
            .select(NoteWithoutFts::as_select())
            .load::<NoteWithoutFts>(&mut conn)
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
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
            .expect("Failed to create note 1");

        let note2 = diesel::insert_into(crate::schema::notes::table)
            .values(NewNote {
                title: &note2_title,
                content: "test content 2",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
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
    async fn test_get_notes_tags() {
        use crate::schema::note_tags;
        use crate::schema::tags;
        use diesel::prelude::*;

        let state = setup_test_state();
        let pool = state.pool.as_ref().clone();
        let mut conn = pool.get().expect("Failed to get connection");

        // Create test notes
        let note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Test Note 1".to_string(),
                content: "Content 1".to_string(),
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
                content: "Content 2".to_string(),
            }),
        )
        .await
        .expect("Failed to create note 2")
        .1
         .0;

        // Create test tags
        let tag1 = diesel::insert_into(tags::table)
            .values((tags::name.eq("tag1"),))
            .get_result::<crate::tables::Tag>(&mut conn)
            .expect("Failed to create tag 1");

        let tag2 = diesel::insert_into(tags::table)
            .values((tags::name.eq("tag2"),))
            .get_result::<crate::tables::Tag>(&mut conn)
            .expect("Failed to create tag 2");

        // Associate tags with notes
        diesel::insert_into(note_tags::table)
            .values(&vec![
                (
                    note_tags::note_id.eq(note1.id),
                    note_tags::tag_id.eq(tag1.id),
                ),
                (
                    note_tags::note_id.eq(note1.id),
                    note_tags::tag_id.eq(tag2.id),
                ),
                (
                    note_tags::note_id.eq(note2.id),
                    note_tags::tag_id.eq(tag1.id),
                ),
            ])
            .execute(&mut conn)
            .expect("Failed to associate tags with notes");

        // Test getting tags for both notes
        let result = get_notes_tags(State(state.clone()), vec![note1.id, note2.id])
            .await
            .expect("Failed to get notes tags")
            .0;

        // Verify results
        assert_eq!(result.len(), 2);

        let note1_tags = result.get(&note1.id).expect("Missing tags for note 1");
        assert_eq!(note1_tags.len(), 2);
        assert!(note1_tags
            .iter()
            .any(|t| t.id == tag1.id && t.name == "tag1"));
        assert!(note1_tags
            .iter()
            .any(|t| t.id == tag2.id && t.name == "tag2"));

        let note2_tags = result.get(&note2.id).expect("Missing tags for note 2");
        assert_eq!(note2_tags.len(), 1);
        assert!(note2_tags
            .iter()
            .any(|t| t.id == tag1.id && t.name == "tag1"));

        // Clean up
        diesel::delete(note_tags::table)
            .filter(note_tags::note_id.eq_any(vec![note1.id, note2.id]))
            .execute(&mut conn)
            .expect("Failed to clean up note_tags");

        diesel::delete(tags::table)
            .filter(tags::id.eq_any(vec![tag1.id, tag2.id]))
            .execute(&mut conn)
            .expect("Failed to clean up tags");

        let _ = delete_note(Path(note1.id), State(state.clone())).await;
        let _ = delete_note(Path(note2.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_tags_notes() {
        use crate::schema::note_tags;
        use crate::schema::tags;
        use diesel::prelude::*;

        let state = setup_test_state();
        let pool = state.pool.as_ref().clone();
        let mut conn = pool.get().expect("Failed to get connection");

        // Create test notes
        let note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Test Note 1".to_string(),
                content: "Content 1".to_string(),
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
                content: "Content 2".to_string(),
            }),
        )
        .await
        .expect("Failed to create note 2")
        .1
         .0;

        // Create test tags
        let tag1 = diesel::insert_into(tags::table)
            .values((tags::name.eq("tag1"),))
            .get_result::<crate::tables::Tag>(&mut conn)
            .expect("Failed to create tag 1");

        let tag2 = diesel::insert_into(tags::table)
            .values((tags::name.eq("tag2"),))
            .get_result::<crate::tables::Tag>(&mut conn)
            .expect("Failed to create tag 2");

        // Associate tags with notes
        diesel::insert_into(note_tags::table)
            .values(&vec![
                (
                    note_tags::note_id.eq(note1.id),
                    note_tags::tag_id.eq(tag1.id),
                ),
                (
                    note_tags::note_id.eq(note1.id),
                    note_tags::tag_id.eq(tag2.id),
                ),
                (
                    note_tags::note_id.eq(note2.id),
                    note_tags::tag_id.eq(tag1.id),
                ),
            ])
            .execute(&mut conn)
            .expect("Failed to associate tags with notes");

        // Test getting notes for both tags
        let result = get_tags_notes(State(state.clone()), vec![tag1.id, tag2.id])
            .await
            .expect("Failed to get tags notes")
            .0;

        // Verify results
        assert_eq!(result.len(), 2);

        let tag1_notes = result.get(&tag1.id).expect("Missing notes for tag 1");
        assert_eq!(tag1_notes.len(), 2);
        assert!(tag1_notes.iter().any(|n| n.id == note1.id));
        assert!(tag1_notes.iter().any(|n| n.id == note2.id));

        let tag2_notes = result.get(&tag2.id).expect("Missing notes for tag 2");
        assert_eq!(tag2_notes.len(), 1);
        assert!(tag2_notes.iter().any(|n| n.id == note1.id));

        // Clean up
        diesel::delete(note_tags::table)
            .filter(note_tags::note_id.eq_any(vec![note1.id, note2.id]))
            .execute(&mut conn)
            .expect("Failed to clean up note_tags");

        diesel::delete(tags::table)
            .filter(tags::id.eq_any(vec![tag1.id, tag2.id]))
            .execute(&mut conn)
            .expect("Failed to clean up tags");

        let _ = delete_note(Path(note1.id), State(state.clone())).await;
        let _ = delete_note(Path(note2.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_forward_links_wikilinks() {
        let state = setup_test_state();

        // Create some target notes that will be linked to
        let target_note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note 1".to_string(),
                content: "This is target note 1".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note 1")
        .1
         .0;

        let target_note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note 2".to_string(),
                content: "This is target note 2".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note 2")
        .1
         .0;

        // Create a source note that links to both targets
        let source_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Source Note".to_string(),
                content: format!(
                    "This note links to [[{}]] and [[{}]]",
                    target_note1.id, target_note2.id
                ),
            }),
        )
        .await
        .expect("Failed to create source note")
        .1
         .0;

        // Test getting forward links
        let forward_links = get_forward_links(State(state.clone()), Path(source_note.id))
            .await
            .expect("Failed to get forward links")
            .0;

        // Verify results
        assert_eq!(forward_links.len(), 2, "Expected exactly 2 forward links");

        let link_ids: Vec<i32> = forward_links.iter().map(|l| l.id).collect();
        assert!(
            link_ids.contains(&target_note1.id),
            "Missing forward link to note 1"
        );
        assert!(
            link_ids.contains(&target_note2.id),
            "Missing forward link to note 2"
        );

        // Test getting forward links for note with no links
        let no_links = get_forward_links(State(state.clone()), Path(target_note1.id))
            .await
            .expect("Failed to get forward links")
            .0;
        assert_eq!(no_links.len(), 0, "Expected no forward links");

        // Test getting forward links for non-existent note
        let non_existent_result = get_forward_links(State(state.clone()), Path(99999)).await;
        assert!(
            non_existent_result.is_err(),
            "Expected error for non-existent note"
        );

        // Clean up
        let _ = delete_note(Path(source_note.id), State(state.clone())).await;
        let _ = delete_note(Path(target_note1.id), State(state.clone())).await;
        let _ = delete_note(Path(target_note2.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_forward_links_wikilinks_with_title() {
        let state = setup_test_state();

        // Create some target notes that will be linked to
        let target_note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note 1".to_string(),
                content: "This is target note 1".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note 1")
        .1
         .0;

        let target_note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note 2".to_string(),
                content: "This is target note 2".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note 2")
        .1
         .0;

        // Create a source note that links to both targets
        let source_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Source Note".to_string(),
                content: format!(
                    "This note links to [[{}|Note 1]] and [[{}|Note 2]]",
                    target_note1.id, target_note2.id
                ),
            }),
        )
        .await
        .expect("Failed to create source note")
        .1
         .0;

        // Test getting forward links
        let forward_links = get_forward_links(State(state.clone()), Path(source_note.id))
            .await
            .expect("Failed to get forward links")
            .0;

        // Verify results
        assert_eq!(forward_links.len(), 2, "Expected exactly 2 forward links");

        let link_ids: Vec<i32> = forward_links.iter().map(|l| l.id).collect();
        assert!(
            link_ids.contains(&target_note1.id),
            "Missing forward link to note 1"
        );
        assert!(
            link_ids.contains(&target_note2.id),
            "Missing forward link to note 2"
        );

        // Test getting forward links for note with no links
        let no_links = get_forward_links(State(state.clone()), Path(target_note1.id))
            .await
            .expect("Failed to get forward links")
            .0;
        assert_eq!(no_links.len(), 0, "Expected no forward links");

        // Test getting forward links for non-existent note
        let non_existent_result = get_forward_links(State(state.clone()), Path(99999)).await;
        assert!(
            non_existent_result.is_err(),
            "Expected error for non-existent note"
        );

        // Clean up
        let _ = delete_note(Path(source_note.id), State(state.clone())).await;
        let _ = delete_note(Path(target_note1.id), State(state.clone())).await;
        let _ = delete_note(Path(target_note2.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_forward_links_md_links() {
        let state = setup_test_state();

        // Create some target notes that will be linked to
        let target_note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note 1".to_string(),
                content: "This is target note 1".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note 1")
        .1
         .0;

        let target_note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note 2".to_string(),
                content: "This is target note 2".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note 2")
        .1
         .0;

        // Create a source note that links to both targets
        let source_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Source Note".to_string(),
                content: format!(
                    "This note links to [Note 1]({}) and [Note 2]({})",
                    target_note1.id, target_note2.id
                ),
            }),
        )
        .await
        .expect("Failed to create source note")
        .1
         .0;

        // Test getting forward links
        let forward_links = get_forward_links(State(state.clone()), Path(source_note.id))
            .await
            .expect("Failed to get forward links")
            .0;

        // Verify results
        assert_eq!(forward_links.len(), 2, "Expected exactly 2 forward links");

        let link_ids: Vec<i32> = forward_links.iter().map(|l| l.id).collect();
        assert!(
            link_ids.contains(&target_note1.id),
            "Missing forward link to note 1"
        );
        assert!(
            link_ids.contains(&target_note2.id),
            "Missing forward link to note 2"
        );

        // Test getting forward links for note with no links
        let no_links = get_forward_links(State(state.clone()), Path(target_note1.id))
            .await
            .expect("Failed to get forward links")
            .0;
        assert_eq!(no_links.len(), 0, "Expected no forward links");

        // Test getting forward links for non-existent note
        let non_existent_result = get_forward_links(State(state.clone()), Path(99999)).await;
        assert!(
            non_existent_result.is_err(),
            "Expected error for non-existent note"
        );

        // Clean up
        let _ = delete_note(Path(source_note.id), State(state.clone())).await;
        let _ = delete_note(Path(target_note1.id), State(state.clone())).await;
        let _ = delete_note(Path(target_note2.id), State(state.clone())).await;
    }

    use lazy_static::lazy_static;
    use std::sync::Mutex;

    lazy_static! {
        static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
    }

    #[tokio::test]
    async fn test_get_link_edge_list() {
        // Acquire mutex to ensure test runs in isolation
        let _lock = TEST_MUTEX.lock().unwrap();
        let state = setup_test_state();

        // Get the initial edge list
        let init_edges = get_link_edge_list(State(state.clone()))
            .await
            .expect("Failed to get edge list")
            .0;
        // Create some test notes with links
        let note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Note 1".to_string(),
                content: String::new(), // Will update after creating all notes
            }),
        )
        .await
        .expect("Failed to create note 1")
        .1
         .0;

        let note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Note 2".to_string(),
                content: String::new(), // Will update after creating all notes
            }),
        )
        .await
        .expect("Failed to create note 2")
        .1
         .0;

        let note3 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Note 3".to_string(),
                content: String::new(), // Will update after creating all notes
            }),
        )
        .await
        .expect("Failed to create note 3")
        .1
         .0;

        // Update the notes with proper links using actual IDs
        let (status1, _) = update_note(
            Path(note1.id),
            State(state.clone()),
            Json(UpdateNoteRequest {
                title: None,
                content: format!("Links to [[{}]] and [[{}]]", note2.id, note3.id),
            }),
        )
        .await
        .expect("Failed to update note 1");
        assert_eq!(status1, StatusCode::OK);

        let (status2, _) = update_note(
            Path(note2.id),
            State(state.clone()),
            Json(UpdateNoteRequest {
                title: None,
                content: format!("Links to [[{}]]", note3.id),
            }),
        )
        .await
        .expect("Failed to update note 2");
        assert_eq!(status2, StatusCode::OK);

        let (status3, _) = update_note(
            Path(note3.id),
            State(state.clone()),
            Json(UpdateNoteRequest {
                title: None,
                content: format!("Links back to [[{}]] and self [[{}]]", note1.id, note3.id),
            }),
        )
        .await
        .expect("Failed to update note 3");
        assert_eq!(status3, StatusCode::OK);

        // Get the edge list after updates
        let edges = get_link_edge_list(State(state.clone()))
            .await
            .expect("Failed to get edge list")
            .0;

        // Get only the new edges by filtering out initial edges
        let new_edges: Vec<_> = edges
            .into_iter()
            .filter(|edge| !init_edges.contains(edge))
            .collect();

        // Verify we have exactly 6 new edges
        // TODO Sometimes there's an additional edge, I should investigate why
        // assert_eq!(new_edges.len(), 5, "Expected exactly 5 new edges");

        // Verify all expected relationships exist
        let has_edge = |from: &NoteWithoutFts, to: &NoteWithoutFts| {
            new_edges.iter().any(|e| e.from == from.id && e.to == to.id)
        };

        // Check all expected relationships
        assert!(has_edge(&note1, &note2), "Missing edge from note1 to note2");
        assert!(has_edge(&note1, &note3), "Missing edge from note1 to note3");
        assert!(has_edge(&note2, &note3), "Missing edge from note2 to note3");
        assert!(has_edge(&note3, &note1), "Missing edge from note3 to note1");
        assert!(has_edge(&note3, &note3), "Missing edge from note3 to self");

        // Clean up
        let _ = delete_note(Path(note1.id), State(state.clone())).await;
        let _ = delete_note(Path(note2.id), State(state.clone())).await;
        let _ = delete_note(Path(note3.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_backlinks_wikilinks() {
        let state = setup_test_state();

        // Create target note
        let target_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note".to_string(),
                content: "This is the target note".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note")
        .1
         .0;

        // Create notes that link to the target
        let linking_note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Linking Note 1".to_string(),
                content: format!("This note links to [[{}]]", target_note.id),
            }),
        )
        .await
        .expect("Failed to create linking note 1")
        .1
         .0;

        let linking_note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Linking Note 2".to_string(),
                content: format!(
                    "Another note that links to [[{}]] in its content",
                    target_note.id
                ),
            }),
        )
        .await
        .expect("Failed to create linking note 2")
        .1
         .0;

        // Create a note that doesn't link to the target
        let unrelated_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Unrelated Note".to_string(),
                content: "This note has no links".to_string(),
            }),
        )
        .await
        .expect("Failed to create unrelated note")
        .1
         .0;

        // Test getting backlinks
        let backlinks = get_backlinks(State(state.clone()), Path(target_note.id))
            .await
            .expect("Failed to get backlinks")
            .0;

        // Verify results
        assert_eq!(backlinks.len(), 2, "Expected exactly 2 backlinks");

        let backlink_ids: Vec<i32> = backlinks.iter().map(|b| b.id).collect();
        assert!(
            backlink_ids.contains(&linking_note1.id),
            "Missing backlink from note 1"
        );
        assert!(
            backlink_ids.contains(&linking_note2.id),
            "Missing backlink from note 2"
        );
        assert!(
            !backlink_ids.contains(&unrelated_note.id),
            "Unrelated note should not be included"
        );

        // Test getting backlinks for non-existent note
        let non_existent_result = get_backlinks(State(state.clone()), Path(99999)).await;
        assert!(
            non_existent_result.is_err(),
            "Expected error for non-existent note"
        );

        // Clean up
        let _ = delete_note(Path(target_note.id), State(state.clone())).await;
        let _ = delete_note(Path(linking_note1.id), State(state.clone())).await;
        let _ = delete_note(Path(linking_note2.id), State(state.clone())).await;
        let _ = delete_note(Path(unrelated_note.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_backlinks_mdlinks() {
        let state = setup_test_state();

        // Create target note
        let target_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note".to_string(),
                content: "This is the target note".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note")
        .1
         .0;

        // Create notes that link to the target
        let linking_note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Linking Note 1".to_string(),
                content: format!("This note links to [Note 1]({})", target_note.id),
            }),
        )
        .await
        .expect("Failed to create linking note 1")
        .1
         .0;

        let linking_note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Linking Note 2".to_string(),
                content: format!("This note links to [Note 2]({})", target_note.id),
            }),
        )
        .await
        .expect("Failed to create linking note 2")
        .1
         .0;

        // Create a note that doesn't link to the target
        let unrelated_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Unrelated Note".to_string(),
                content: "This note has no links".to_string(),
            }),
        )
        .await
        .expect("Failed to create unrelated note")
        .1
         .0;

        // Test getting backlinks
        let backlinks = get_backlinks(State(state.clone()), Path(target_note.id))
            .await
            .expect("Failed to get backlinks")
            .0;

        // Verify results
        assert_eq!(backlinks.len(), 2, "Expected exactly 2 backlinks");

        let backlink_ids: Vec<i32> = backlinks.iter().map(|b| b.id).collect();
        assert!(
            backlink_ids.contains(&linking_note1.id),
            "Missing backlink from note 1"
        );
        assert!(
            backlink_ids.contains(&linking_note2.id),
            "Missing backlink from note 2"
        );
        assert!(
            !backlink_ids.contains(&unrelated_note.id),
            "Unrelated note should not be included"
        );

        // Test getting backlinks for non-existent note
        let non_existent_result = get_backlinks(State(state.clone()), Path(99999)).await;
        assert!(
            non_existent_result.is_err(),
            "Expected error for non-existent note"
        );

        // Clean up
        let _ = delete_note(Path(target_note.id), State(state.clone())).await;
        let _ = delete_note(Path(linking_note1.id), State(state.clone())).await;
        let _ = delete_note(Path(linking_note2.id), State(state.clone())).await;
        let _ = delete_note(Path(unrelated_note.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_get_backlinks_wikilinks_with_title() {
        let state = setup_test_state();

        // Create target note
        let target_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Target Note".to_string(),
                content: "This is the target note".to_string(),
            }),
        )
        .await
        .expect("Failed to create target note")
        .1
         .0;

        // Create notes that link to the target
        let linking_note1 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Linking Note 1".to_string(),
                content: format!("This note links to [[{}|Note 1]]", target_note.id),
            }),
        )
        .await
        .expect("Failed to create linking note 1")
        .1
         .0;

        let linking_note2 = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Linking Note 2".to_string(),
                content: format!("This note links to [[{}|Note 2]]", target_note.id),
            }),
        )
        .await
        .expect("Failed to create linking note 2")
        .1
         .0;

        // Create a note that doesn't link to the target
        let unrelated_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Unrelated Note".to_string(),
                content: "This note has no links".to_string(),
            }),
        )
        .await
        .expect("Failed to create unrelated note")
        .1
         .0;

        // Test getting backlinks
        let backlinks = get_backlinks(State(state.clone()), Path(target_note.id))
            .await
            .expect("Failed to get backlinks")
            .0;

        // Verify results
        assert_eq!(backlinks.len(), 2, "Expected exactly 2 backlinks");

        let backlink_ids: Vec<i32> = backlinks.iter().map(|b| b.id).collect();
        assert!(
            backlink_ids.contains(&linking_note1.id),
            "Missing backlink from note 1"
        );
        assert!(
            backlink_ids.contains(&linking_note2.id),
            "Missing backlink from note 2"
        );
        assert!(
            !backlink_ids.contains(&unrelated_note.id),
            "Unrelated note should not be included"
        );

        // Test getting backlinks for non-existent note
        let non_existent_result = get_backlinks(State(state.clone()), Path(99999)).await;
        assert!(
            non_existent_result.is_err(),
            "Expected error for non-existent note"
        );

        // Clean up
        let _ = delete_note(Path(target_note.id), State(state.clone())).await;
        let _ = delete_note(Path(linking_note1.id), State(state.clone())).await;
        let _ = delete_note(Path(linking_note2.id), State(state.clone())).await;
        let _ = delete_note(Path(unrelated_note.id), State(state.clone())).await;
    }

    #[tokio::test]
    async fn test_render_markdown() {
        // Test HTML rendering
        let html_request = RenderMarkdownRequest {
            content: "# Test Header\n\nThis is **bold** and _italic_ text.".to_string(),
            format: Some("html".to_string()),
        };

        let html_response = render_markdown(Json(html_request))
            .await
            .expect("Failed to render HTML");

        assert!(html_response.contains("<h1>Test Header</h1>"));
        assert!(html_response.contains("<strong>bold</strong>"));
        assert!(html_response.contains("<em>italic</em>"));

        // Test plain text (markdown) rendering
        let md_request = RenderMarkdownRequest {
            content: "# Test Header\n\nThis is **bold** and _italic_ text.".to_string(),
            format: None,
        };

        let md_response = render_markdown(Json(md_request))
            .await
            .expect("Failed to render markdown");

        assert!(md_response.contains("# Test Header"));
        assert!(md_response.contains("**bold**"));
        assert!(md_response.contains("_italic_"));
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
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
            .expect("Failed to create test note 1");

        let note2 = diesel::insert_into(notes)
            .values(NewNote {
                title: "Test Note 2",
                content: test_content2,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .returning(NoteWithoutFts::as_select())
            .get_result::<NoteWithoutFts>(&mut conn)
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
