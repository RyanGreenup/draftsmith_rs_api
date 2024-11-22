use super::generics::{
    attach_child, build_generic_tree, detach_child, is_circular_hierarchy, BasicTreeNode,
    HierarchyItem,
};
use crate::api::{
    get_connection, get_note_content, get_notes_tags, state::AppState, tags::TagResponse, Path,
};
use crate::tables::NewNoteTag;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::{HashMap, HashSet};

lazy_static! {
    static ref LINK_REGEX: Regex = Regex::new(r"\[\[(\d+)(?:\|([^\]]+))?\]\]").unwrap();
}

#[derive(Debug, serde::Deserialize)]
pub struct AttachChildNoteRequest {
    pub parent_note_id: Option<i32>,
    pub child_note_id: i32,
}
use crate::tables::{NewNote, NewNoteHierarchy, NoteHierarchy, NoteWithoutFts};
use diesel::prelude::*;

impl HierarchyItem for NoteHierarchy {
    type Id = i32;

    fn get_parent_id(&self) -> Option<i32> {
        self.parent_note_id
    }

    fn get_child_id(&self) -> i32 {
        self.child_note_id
            .expect("child_note_id should not be None")
    }

    fn set_parent_id(&mut self, parent_id: Option<i32>) {
        self.parent_note_id = parent_id;
    }

    fn set_child_id(&mut self, child_id: i32) {
        self.child_note_id = Some(child_id);
    }

    fn find_by_child_id(conn: &mut PgConnection, child_id: i32) -> QueryResult<Option<Self>> {
        use crate::schema::note_hierarchy::dsl::*;

        note_hierarchy
            .filter(child_note_id.eq(child_id))
            .first::<NoteHierarchy>(conn)
            .optional()
    }

    fn insert_new(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::note_hierarchy;

        let new_item = NewNoteHierarchy {
            parent_note_id: item.parent_note_id,
            child_note_id: item.child_note_id,
        };

        diesel::insert_into(note_hierarchy::table)
            .values(&new_item)
            .execute(conn)
            .map(|_| ())
    }

    fn update_existing(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::note_hierarchy::dsl::*;

        diesel::update(note_hierarchy.filter(child_note_id.eq(item.get_child_id())))
            .set(parent_note_id.eq(item.get_parent_id()))
            .execute(conn)
            .map(|_| ())
    }
}
use axum::{
    debug_handler,
    extract::{Json, State},
    http::StatusCode,
};
use serde::Serialize;

#[derive(Debug, serde::Deserialize, Serialize, Clone)]
pub struct NoteTreeNode {
    pub id: i32,
    pub title: Option<String>,
    pub content: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub children: Vec<NoteTreeNode>,
    pub tags: Vec<TagResponse>,
}

// Modify Note Hierarchy

pub async fn attach_child_note(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildNoteRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Define function to get parent ID from note hierarchy
    let get_parent_fn = |conn: &mut PgConnection, child_id: i32| -> QueryResult<Option<i32>> {
        use crate::schema::note_hierarchy::dsl::*;
        note_hierarchy
            .filter(child_note_id.eq(child_id))
            .select(parent_note_id)
            .first::<Option<i32>>(conn)
            .optional()
            .map(|opt| opt.flatten())
    };

    // Define the is_circular function specific to notes
    let is_circular_fn = |conn: &mut PgConnection, child_id: i32, parent_id: Option<i32>| {
        is_circular_hierarchy(conn, child_id, parent_id, get_parent_fn)
    };

    // Create a NoteHierarchy item
    let item = NoteHierarchy {
        id: 0, // Assuming 'id' is auto-generated
        parent_note_id: payload.parent_note_id,
        child_note_id: Some(payload.child_note_id),
    };

    // Call the generic attach_child function with the specific implementation
    attach_child(is_circular_fn, item, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[debug_handler]
pub async fn get_note_tree(
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

    // Prepare data for generic tree building
    let note_data: Vec<(i32, NoteWithoutFts)> =
        all_notes.into_iter().map(|note| (note.id, note)).collect();

    let hierarchy_tuples: Vec<(i32, i32)> = hierarchies
        .iter()
        .filter_map(|h| h.child_note_id.zip(h.parent_note_id))
        .collect();

    // Build the basic tree
    let basic_tree = build_generic_tree(&note_data, &hierarchy_tuples);

    // Convert BasicTreeNode to NoteTreeNode
    async fn convert_to_note_tree(
        basic_node: BasicTreeNode<NoteWithoutFts>,
        state: &AppState,
    ) -> NoteTreeNode {
        NoteTreeNode {
            id: basic_node.id,
            title: Some(basic_node.data.title),
            content: Some(basic_node.data.content),
            created_at: basic_node.data.created_at,
            modified_at: basic_node.data.modified_at,
            children: futures::future::join_all(
                basic_node
                    .children
                    .into_iter()
                    .map(|child| convert_to_note_tree(child, state)),
            )
            .await,
            tags: get_notes_tags(State(state.clone()), vec![basic_node.id])
                .await
                .unwrap_or_default()
                .get(&basic_node.id)
                .cloned()
                .unwrap_or_default(),
        }
    }

    let tree = futures::future::join_all(
        basic_tree
            .into_iter()
            .map(|node| convert_to_note_tree(node, &state)),
    )
    .await;

    Ok(Json(tree))
}

/// Get all note paths
#[debug_handler]
pub async fn get_all_note_paths() -> Result<Json<HashMap<i32, String>>, StatusCode> {
    get_note_paths().await.map(Json)
}

/// Get path for a single note
#[debug_handler]
pub async fn get_single_note_path(Path(note_id): Path<i32>) -> Result<String, StatusCode> {
    get_note_path(&note_id, None)
}

/// Get relative path from one note to another
#[debug_handler]
pub async fn get_relative_note_path(
    Path((note_id, from_id)): Path<(i32, i32)>,
) -> Result<String, StatusCode> {
    get_note_path(&note_id, Some(&from_id))
}

async fn get_note_paths() -> Result<HashMap<i32, String>, StatusCode> {
    let all_components = get_all_note_path_components()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Create a vector of futures
    let path_futures: Vec<_> = all_components
        .into_iter()
        .map(|(id, components)| async move {
            let path = build_hierarchy_path(components, false).to_string();
            (id, path)
        })
        .collect();

    // Wait for all futures to complete
    let paths = futures::future::join_all(path_futures).await;

    // Collect the results into a HashMap
    Ok(paths.into_iter().collect())
}

fn get_note_path(id: &i32, from_id: Option<&i32>) -> Result<String, StatusCode> {
    let (components, relative) = get_note_path_components(id, from_id).map_err(|e| match e {
        diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    })?;

    let path = build_hierarchy_path(components, relative);
    Ok(path)
}

/// Constructs a hierarchy path string from a vector of path items.
///
/// # Arguments
///
/// * `path_items` - A vector of strings representing each item in the hierarchy.
/// * `relative` - A boolean indicating whether the path should be relative or not.
///
/// # Returns
///
/// A `String` that represents the constructed hierarchy path. Each item in the hierarchy is separated by " / ".
///
/// # Examples
///
/// ```rust,ignore
/// use rust_cli_app::api::hierarchy::notes::build_hierarchy_path;
///
/// let path = build_hierarchy_path(vec!["root".to_string(), "folder".to_string(), "file.txt".to_string()], false);
/// assert_eq!(path, "/ root / folder / file.txt");
/// ```
fn build_hierarchy_path(path_items: Vec<String>, relative: bool) -> String {
    if relative {
        path_items.join(" / ")
    } else {
        format!("/ {}", path_items.join(" / "))
    }
}

// Includes a boolean indicating if the path is trimmed to be relative
// NOTE this Return a vector as it makes tests simpler
fn get_note_path_components(
    id: &i32,
    from_id: Option<&i32>,
) -> Result<(Vec<String>, bool), diesel::result::Error> {
    let mut conn = get_connection();
    let mut path_components: Vec<String> = Vec::new();
    let mut path_ids = Vec::new(); // Store IDs to check for from_id
    let mut current_id = *id;

    // Build path from target to root
    loop {
        // Get the current note's title
        let title = {
            use crate::schema::notes::dsl::*;
            // Change this to return early if note not found
            match notes
                .find(current_id)
                .select(title)
                .first::<String>(&mut conn)
            {
                Ok(t) => t,
                Err(diesel::result::Error::NotFound) => {
                    return Err(diesel::result::Error::NotFound);
                }
                Err(e) => return Err(e),
            }
        };

        path_components.push(title);
        path_ids.push(current_id);

        // Look up parent
        let parent_id = {
            use crate::schema::note_hierarchy::dsl::*;
            match note_hierarchy
                .filter(child_note_id.eq(current_id))
                .select(parent_note_id)
                .first::<Option<i32>>(&mut conn)
                .optional()
                .unwrap_or(None)
                .flatten()
            {
                Some(pid) => pid,
                None => break,
            }
        };

        current_id = parent_id;
    }

    // Reverse both vectors since we collected from child to parent
    path_components.reverse();
    path_ids.reverse();

    // If from_id is specified, try to find it in the path
    if let Some(from_id) = from_id {
        if let Some(pos) = path_ids.iter().position(|&id| id == *from_id) {
            // If from_id is found in the path, return only components after it
            let cut_path_components = path_components.split_off(pos + 1);
            // If it's empty, return the full path as it's the same as the target, or from_id is not an ancestor
            if !cut_path_components.is_empty() {
                return Ok((cut_path_components, true));
            }
        };
    }

    // Return full path if from_id is not specified or not found in path
    Ok((path_components, false))
}

async fn get_all_note_path_components() -> Result<HashMap<i32, Vec<String>>, diesel::result::Error>
{
    let mut conn = get_connection();
    let mut paths: HashMap<i32, Vec<String>> = HashMap::new();
    let mut note_cache: HashMap<i32, String> = HashMap::new();
    let mut hierarchy_cache: HashMap<i32, Option<i32>> = HashMap::new();

    // SELECT id, title FROM notes
    // in a single query (get_note_from_path only queries for single note,
    // repeating this logic is more performant)
    {
        use crate::schema::notes::dsl::*;
        let all_notes = notes.select((id, title)).load::<(i32, String)>(&mut conn)?;
        note_cache.extend(all_notes);
    }

    // Then, get all hierarchical relationships in one query
    {
        use crate::schema::note_hierarchy::dsl::*;
        let all_hierarchies = note_hierarchy
            .select((child_note_id, parent_note_id))
            .load::<(Option<i32>, Option<i32>)>(&mut conn)?;

        hierarchy_cache.extend(
            all_hierarchies
                .into_iter()
                .filter_map(|(child, parent)| child.map(|c| (c, parent))),
        );
    }

    // Helper function to build path for a single note using cached data
    fn build_note_path(
        note_id: i32,
        note_cache: &HashMap<i32, String>,
        hierarchy_cache: &HashMap<i32, Option<i32>>,
        paths: &mut HashMap<i32, Vec<String>>,
    ) -> Vec<String> {
        // Return cached path if available
        if let Some(path) = paths.get(&note_id) {
            return path.clone();
        }

        let mut current_path = Vec::new();
        let mut current_id = note_id;

        // Build path from current note to root
        while let Some(title) = note_cache.get(&current_id) {
            current_path.push(title.clone());
        
            // Look up parent and break if none found
            if let Some(parent_id) = hierarchy_cache.get(&current_id).and_then(|&x| x) {
                current_id = parent_id;
            } else {
                break;
            }
        }

        // Reverse path since we collected from child to parent
        current_path.reverse();

        // Cache and return the path
        paths.insert(note_id, current_path.clone());
        current_path
    }

    // Build paths for all notes
    for &note_id in note_cache.keys() {
        if !paths.contains_key(&note_id) {
            build_note_path(note_id, &note_cache, &hierarchy_cache, &mut paths);
        }
    }

    Ok(paths)
}

/// Replaces internal note links in the content of a given note with their respective titles.
///
/// The function processes the content of a specified note (identified by `note_id`) to find and replace
/// links in two formats:
/// - `[[id]]`: These are replaced with `[Note Title](id)`.
/// - `[[id|title]]`: These are replaced with `[Custom Title](id)`, using the custom title if provided.
///
/// This is particularly useful for dynamically updating links when note titles change, ensuring that
/// all references to notes remain accurate and up-to-date. If a link points to a note under a parent,
/// the path will be made relative.
///
/// # Arguments
///
/// * `note_id` - The ID of the note whose content needs processing.
///
/// # Returns
///
/// A `Result<String, StatusCode>` which is:
/// - `Ok(String)`: The processed content with all internal links replaced by their respective titles.
/// - `Err(StatusCode)`: An error status code if the initial content fetch fails (e.g., `NOT_FOUND`).
///
/// # Example
///
/// Given a note with ID `1` and content:
/// ```markdown,ignore
/// This is a link to [[2]] and another one to [[3|Custom Title]].
/// ```
///
/// If note `2` has the title "Second Note" and note `3` has the title "Third Note",
/// the function will return:
/// ```markdown,ignore
/// This is a link to [Second Note](2) and another one to [Custom Title](3).
/// ```
///
/// # Error Handling
///
/// The function handles errors in fetching the path of linked notes by skipping those links.
/// If the initial content fetch fails, it returns an appropriate status code.
pub fn get_note_content_and_replace_links(note_id: i32) -> Result<String, diesel::result::Error> {
    let content = get_note_content(note_id)?;

    // Early return if no links found
    if !LINK_REGEX.is_match(&content) {
        return Ok(content);
    }

    let mut new_content = String::new();
    let mut link_positions = Vec::new();
    let mut unique_ids = HashSet::new();
    let mut last_end = 0;

    // Find all the links in the content
    for cap in LINK_REGEX.find_iter(&content) {
        if let Some(cap_text) = content.get(cap.start()..cap.end()) {
            if let Some(captures) = LINK_REGEX.captures(cap_text) {
                if let (Some(_id_match), Ok(target_id)) =
                    (captures.get(1), captures[1].parse::<i32>())
                {
                    let custom_title = captures.get(2).map(|m| m.as_str().to_string());
                    link_positions.push((cap.start(), cap.end(), target_id, custom_title));
                    unique_ids.insert(target_id);
                }
            }
        }
    }

    // Early return if no valid links found
    if link_positions.is_empty() {
        return Ok(content);
    }

    // Pre-allocate string with estimated capacity
    new_content.reserve(content.len() + link_positions.len() * 20); // Estimate 20 chars per path

    const LINKS_PER_BATCH: usize = 10;

    // Process links in batches
    for chunk in link_positions.chunks(LINKS_PER_BATCH) {
        let mut path_cache = HashMap::with_capacity(chunk.len());

        // Batch process paths for this chunk
        for &(_, _, target_id, _) in chunk {
            let (components, relative) = get_note_path_components(&target_id, Some(&note_id))?;
            let path = if relative {
                components.join(" / ")
            } else {
                format!("/ {}", components.join(" / "))
            };
            path_cache.insert(target_id, path);
        }

        // Build the content for this batch
        for &(start, end, target_id, ref custom_title) in chunk {
            let path = path_cache.get(&target_id).unwrap();
            new_content.push_str(&content[last_end..start]);

            // Use custom title if provided, otherwise use path
            if let Some(title) = custom_title {
                new_content.push_str(&format!("[{}]({})", title, target_id));
            } else {
                new_content.push_str(&format!("[{}]({})", path, target_id));
            }

            last_end = end;
        }
    }

    // Add remaining content after the last match
    new_content.push_str(&content[last_end..]);

    Ok(new_content)
}

// Handler for the PUT /notes/tree endpoint
#[debug_handler]

pub async fn detach_child_note(
    State(state): State<AppState>,
    Path(child_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use crate::schema::note_hierarchy::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Define specific delete logic for the note hierarchy
    let delete_fn = |conn: &mut PgConnection, child_id: i32| {
        diesel::delete(note_hierarchy.filter(child_note_id.eq(child_id))).execute(conn)
    };

    detach_child(delete_fn, child_id, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}
pub async fn update_note_tree(
    State(state): State<AppState>,
    Json(note_trees): Json<Vec<NoteTreeNode>>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state.pool.get().map_err(|e| {
        eprintln!("Failed to get connection: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Process nodes iteratively using a stack
    #[derive(Debug)]
    struct NodeWithParent {
        node: NoteTreeNode,
        parent_id: Option<i32>,
    }

    use crate::schema::note_hierarchy::dsl::{child_note_id, note_hierarchy};
    use crate::schema::note_tags;
    use crate::schema::notes::dsl::{content, id as notes_id, modified_at, notes, title};

    // Initialize stack with root nodes
    let mut stack: Vec<NodeWithParent> = note_trees
        .into_iter()
        .map(|node| NodeWithParent {
            node,
            parent_id: None,
        })
        .collect();

    // Process nodes while stack is not empty
    while let Some(NodeWithParent { node, parent_id }) = stack.pop() {
        eprintln!("Processing node: id={}, title={:?}", node.id, node.title);

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
                .get_result::<i32>(&mut conn);

            match result {
                Ok(other_id) => {
                    eprintln!("Inserted new note with id: {}", other_id);
                    other_id
                }
                Err(e) => {
                    eprintln!("Failed to insert new note: {:?}", e);
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
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
                .execute(&mut conn)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            node.id
        };

        // Update hierarchy only if there is a parent
        if let Some(p_id) = parent_id {
            // Remove existing hierarchy entry for this node
            diesel::delete(note_hierarchy.filter(child_note_id.eq(node_id)))
                .execute(&mut conn)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

            // Insert new hierarchy entry
            let new_hierarchy = NewNoteHierarchy {
                child_note_id: Some(node_id),
                parent_note_id: Some(p_id),
            };
            diesel::insert_into(note_hierarchy)
                .values(&new_hierarchy)
                .execute(&mut conn)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Update tags
        // First remove existing tags
        diesel::delete(note_tags::table.filter(note_tags::note_id.eq(node_id)))
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        // Then insert new tags
        if !node.tags.is_empty() {
            let new_tags: Vec<_> = node
                .tags
                .iter()
                .map(|tag| NewNoteTag {
                    note_id: node_id,
                    tag_id: tag.id,
                })
                .collect();

            diesel::insert_into(note_tags::table)
                .values(new_tags)
                .execute(&mut conn)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Add children to stack (in reverse order to maintain same processing order as recursive version)
        for child in node.children.into_iter().rev() {
            stack.push(NodeWithParent {
                node: child,
                parent_id: Some(node_id),
            });
        }
    }

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod note_hierarchy_tests {
    use super::*;
    use crate::api::tests::{setup_test_state, TestCleanup};
    use crate::api::DieselError;
    use crate::api::{create_note, CreateNoteRequest};
    use crate::tables::NoteBad;
    use axum::extract::State;
    use axum::Json;

    /// Tests the function to update notes from a supplied tree structure
    /// This can't use a conn.test_transaction block because
    /// the tree function is recursive and passing in a connection
    /// will add too much complexity to the test.
    /// This function automatically cleans up after itself via Drop trait.
    #[tokio::test]
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
            tags: Vec::new(),
            children: vec![
                NoteTreeNode {
                    id: 0,
                    title: Some("".to_string()),
                    content: Some(child1_content.clone()),
                    created_at: None,
                    modified_at: None,
                    tags: Vec::new(),
                    children: vec![],
                },
                NoteTreeNode {
                    id: 0,
                    title: Some("".to_string()),
                    content: Some(child2_content.clone()),
                    created_at: None,
                    modified_at: None,
                    tags: Vec::new(),
                    children: vec![],
                },
            ],
        };

        // Call the function to update the database
        let response = update_note_tree(State(state.clone()), Json(vec![input_tree])).await;

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
                .load::<NoteBad>(conn)
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
            .get_result::<NoteBad>(&mut conn)
            .expect("Failed to create root note");

        let child1_note = diesel::insert_into(notes)
            .values(NewNote {
                title: &child1_title,
                content: note_1_content_original,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<NoteBad>(&mut conn)
            .expect("Failed to create child1 note");

        let child2_note = diesel::insert_into(notes)
            .values(NewNote {
                title: &child2_title,
                content: note_2_content_original,
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            })
            .get_result::<NoteBad>(&mut conn)
            .expect("Failed to create child2 note");

        // Create initial hierarchy: root -> child1 -> child2
        use crate::schema::note_hierarchy::dsl::*;
        diesel::insert_into(note_hierarchy)
            .values(&NewNoteHierarchy {
                child_note_id: Some(child1_note.id),
                parent_note_id: Some(root_note.id),
            })
            .execute(&mut conn)
            .expect("Failed to create first hierarchy link");

        diesel::insert_into(note_hierarchy)
            .values(&NewNoteHierarchy {
                child_note_id: Some(child2_note.id),
                parent_note_id: Some(child1_note.id),
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
            tags: Vec::new(),
            children: vec![NoteTreeNode {
                id: child2_id,
                title: Some(child2_title),
                content: Some(note_2_content_updated.to_string()),
                created_at: None,
                modified_at: None,
                tags: Vec::new(),
                children: vec![NoteTreeNode {
                    id: child1_id,
                    title: Some(child1_title),
                    content: Some(note_1_content_updated.to_string()),
                    created_at: None,
                    modified_at: None,
                    tags: Vec::new(),
                    children: vec![],
                }],
            }],
        };

        // Update the hierarchy
        let response = update_note_tree(State(state.clone()), Json(vec![modified_tree]))
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

        // Check child1 is now under child2
        let child2_children = note_hierarchy
            .filter(parent_note_id.eq(child2_id))
            .load::<NoteHierarchy>(&mut conn)
            .expect("Failed to load child2 children");
        assert_eq!(child2_children.len(), 1);
        assert_eq!(child2_children[0].child_note_id, Some(child1_id));

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
            .load::<NoteBad>(&mut conn)
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
    async fn test_get_note_path_new() {
        let state = setup_test_state();

        // Create a hierarchy of notes
        // A -> B -> C
        // D -> E
        let notes = vec![
            ("A", None),    // test_id: 0
            ("B", Some(0)), // test_id: 1, parent: A
            ("C", Some(1)), // test_id: 2, parent: B
            ("D", None),    // test_id: 3
            ("E", Some(3)), // test_id: 4, parent: D
        ];

        // Create the notes and store their IDs
        let mut note_ids = Vec::new();
        for (title, _) in &notes {
            let note = create_note(
                State(state.clone()),
                Json(CreateNoteRequest {
                    title: String::new(),
                    content: format!("# {}\n\n", title),
                }),
            )
            .await
            .expect("Failed to create note")
            .1
             .0;
            note_ids.push(note.id);
        }

        // Set up the hierarchy
        for (i, (_, parent_idx)) in notes.iter().enumerate() {
            if let Some(parent_idx) = parent_idx {
                attach_child_note(
                    State(state.clone()),
                    Json(AttachChildNoteRequest {
                        child_note_id: note_ids[i],
                        parent_note_id: Some(note_ids[*parent_idx]),
                    }),
                )
                .await
                .expect("Failed to attach child note");
            }
        }

        // Create cleanup struct that will automatically clean up when dropped
        let _cleanup = TestCleanup {
            pool: state.pool.as_ref().clone(),
            note_ids: note_ids.clone(),
        };

        // Test cases
        let test_cases = vec![
            // Get the path vector of A
            (note_ids[0], None, vec!["A"]),
            // Get the path vector of C
            (note_ids[2], None, vec!["A", "B", "C"]),
            // Get the path vector of C starting from B
            (note_ids[2], Some(note_ids[1]), vec!["C"]),
            // Get the path vector of C starting from A
            (note_ids[2], Some(note_ids[0]), vec!["B", "C"]),
            // Get the path vector of C starting from D (should return full path as D is not a parent)
            (note_ids[2], Some(note_ids[3]), vec!["A", "B", "C"]),
        ];

        for (note_id, from_id, expected_path) in test_cases {
            let (path, _relative) = get_note_path_components(&note_id, from_id.as_ref())
                .expect("Failed to get note path components");

            assert_eq!(
                path, expected_path,
                "Path mismatch for note_id={}, from_id={:?}",
                note_id, from_id
            );
        }
    }

    #[tokio::test]
    async fn test_get_note_content_and_replace_links() {
        let state = setup_test_state();

        // Create a hierarchy of notes
        let root_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "".to_string(),
                content: "# Root".to_string(),
            }),
        )
        .await
        .expect("Failed to create root note")
        .1
         .0;

        let child_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "".to_string(),
                content: "# Child".to_string(),
            }),
        )
        .await
        .expect("Failed to create child note")
        .1
         .0;

        let unrelated_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "".to_string(),
                content: "# Unrelated".to_string(),
            }),
        )
        .await
        .expect("Failed to create unrelated note")
        .1
         .0;

        // Set up hierarchy
        attach_child_note(
            State(state.clone()),
            Json(AttachChildNoteRequest {
                child_note_id: child_note.id,
                parent_note_id: Some(root_note.id),
            }),
        )
        .await
        .expect("Failed to attach child note");

        let before = format!(
            r"
# Test Note

Link to child: [[{child_id}]]

Link to unrelated: [[{unrelated_id}]]

Custom title link: [[{root_id}|Custom]]",
            child_id = child_note.id,
            unrelated_id = unrelated_note.id,
            root_id = root_note.id
        );

        let after = format!(
            r"
# Test Note

Link to child: [/ Root / Child]({child_id})

Link to unrelated: [/ Unrelated]({unrelated_id})

Custom title link: [Custom]({root_id})",
            child_id = child_note.id,
            unrelated_id = unrelated_note.id,
            root_id = root_note.id
        );

        // Create a note with various types of links
        let test_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Test Note".to_string(),
                content: before,
            }),
        )
        .await
        .expect("Failed to create test note")
        .1
         .0;

        dbg!(&test_note);

        // Attach test note under root
        attach_child_note(
            State(state.clone()),
            Json(AttachChildNoteRequest {
                child_note_id: test_note.id,
                parent_note_id: Some(root_note.id),
            }),
        )
        .await
        .expect("Failed to attach test note");

        /*
        - unrelated_note
        - root_note
          - test_note
          - child_note

        So:
        # Test Note

              [[{child_id}]] => [/ Root / Child ]
              [[{root_id}]] => [[Custom|{root_id}]] => [Custom](root_note.id)
              [[{unrelated_id}]] => [/ Unrelated ]
        */

        // Get the processed content
        let processed_content =
            get_note_content_and_replace_links(test_note.id).expect("Failed to process content");

        dbg!(&processed_content);
        dbg!(&after);

        // Verify the links are replaced correctly
        assert!(processed_content.contains(&format!("[/ Root / Child]({})", child_note.id)));
        assert!(processed_content.contains(&format!("[/ Unrelated]({})", unrelated_note.id)));
        assert!(processed_content.contains(&format!("[Custom]({})", root_note.id)));

        // Verify the content looks right
        assert!(processed_content.trim() == after.trim());

        // Clean up
        let _cleanup = TestCleanup {
            pool: state.pool.as_ref().clone(),
            note_ids: vec![root_note.id, child_note.id, unrelated_note.id, test_note.id],
        };
    }
}
