use super::generics::{
    attach_child, build_generic_tree, detach_child, is_circular_hierarchy, BasicTreeNode,
    HierarchyItem,
};
use crate::api::{
    get_connection, get_note_content, get_notes_tags, state::AppState, tags::TagResponse, Path,
};
use crate::tables::NewNoteTag;
use std::collections::HashMap;

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
pub async fn get_all_note_paths(
    State(state): State<AppState>,
) -> Result<Json<HashMap<i32, String>>, StatusCode> {
    get_note_paths(&state).await.map(Json)
}

/// Get path for a single note
#[debug_handler]
pub async fn get_single_note_path(
    State(state): State<AppState>,
    Path(note_id): Path<i32>,
) -> Result<String, StatusCode> {
    get_note_path(&state, &note_id, None).await
}

/// Get relative path from one note to another
#[debug_handler]
pub async fn get_relative_note_path(
    State(state): State<AppState>,
    Path((note_id, from_id)): Path<(i32, i32)>,
) -> Result<String, StatusCode> {
    get_note_path(&state, &note_id, Some(&from_id)).await
}

async fn get_note_paths(state: &AppState) -> Result<HashMap<i32, String>, StatusCode> {
    // Get the full tree structure
    let tree = get_note_tree(State(state.clone())).await?.0;
    let mut paths = HashMap::new();

    // Helper function to recursively build paths
    fn build_paths(node: &NoteTreeNode, current_path: String, paths: &mut HashMap<i32, String>) {
        // Get the node's title, defaulting to "Untitled" if None
        let title = node.title.as_deref().unwrap_or("Untitled");

        // Build the full path for this node
        let node_path = if current_path.is_empty() {
            title.to_string()
        } else {
            format!("{} / {}", current_path, title)
        };

        // Store the path for this node's ID
        // Use a leading / so there's no ambiguity for relative paths
        paths.insert(node.id, format!("/ {}", node_path));

        // Recursively process children
        for child in &node.children {
            build_paths(child, node_path.clone(), paths);
        }
    }

    // Process each root node
    for node in tree {
        build_paths(&node, String::new(), &mut paths);
    }

    Ok(paths)
}

async fn get_note_path(
    state: &AppState,
    id: &i32,
    from_id: Option<&i32>,
) -> Result<String, StatusCode> {
    // Get all paths
    let paths = get_note_paths(state).await?;
    let pullout_path = |id| paths.get(id).ok_or(StatusCode::NOT_FOUND);
    let path = pullout_path(id)?;
    match from_id {
        None => Ok(path.clone()),
        Some(from_id) => {
            // If the from_id is invalid, just return the full path
            let from_path = match pullout_path(from_id) {
                Ok(path) => path,
                // If the from_id is invalid, just return the full path
                Err(_) => return Ok(path.clone()),
            };
            if path.contains(from_path) {
                // Remove the from_path from the path
                let mut trimmed_path = path.replace(from_path, "").trim().to_string();
                // Now remove the leading /
                let leader = "/ ";
                if trimmed_path.starts_with(leader) {
                    trimmed_path = trimmed_path[leader.len()..].to_string();
                }
                if !trimmed_path.is_empty() {
                    Ok(trimmed_path)
                } else {
                    // This (usually) either:
                    //   1. The from_id is the same as the id, and/or
                    //   2. the from_id has the same name as a parent but is not actually a parent
                    //      e.g. Notes that are "Untitled".
                    Ok(path.clone())
                }
            } else {
                // Just return the full path if the id is not under the from_id
                Ok(path.clone())
            }
        }
    }
}

async fn build_hierarchy_path(path_items: Vec<&str>) -> String {
    format!("/{}", path_items.join("/"))
}

async fn get_note_path_new(id: &i32, from_id: Option<&i32>) -> Vec<String> {
    // Get a new database connection
    let mut conn = get_connection();

    // Pull out the note
    let get_note = |note_id| {
        use crate::schema::notes::dsl::*;
        notes.find(note_id).select(title).first::<String>(&mut conn)
    };

    // Create a vector to store the path components
    let mut path_components = Vec::new();
    let mut current_id = *id;

    // Keep looking up parents until we reach the root (no parent)
    while let Ok(title) = get_note(&current_id) {
        path_components.push(title);
        
        // Look up parent using note_hierarchy
        use crate::schema::note_hierarchy::dsl::*;
        match note_hierarchy
            .filter(child_note_id.eq(current_id))
            .select(parent_note_id)
            .first::<Option<i32>>(&mut conn)
            .optional()
            .unwrap_or(None)
            .flatten()
        {
            Some(parent_id) => {
                // Continue with parent
                current_id = parent_id;
            }
            None => {
                // No more parents, break the loop
                break;
            }
        }
    }

    // Reverse the vector since we collected from child to parent
    path_components.reverse();
    
    path_components
}

/// This function replaces links to notes with their title
/// Example:
///     [[1]] -> [Note Title](1)
///     [[33|Custom Title]] -> [Custom Title](33)
/// This is useful for automatically updating links when a note title changes
/// and having dynamic content.
/// If the link is below a parent, it will be relative.
pub async fn get_note_content_and_replace_links(
    state: &AppState,
    note_id: i32,
) -> Result<String, StatusCode> {
    let content = get_note_content(note_id).map_err(|_| StatusCode::NOT_FOUND)?;

    // Regular expression to match both [[id]] and [[id|title]] formats
    let link_regex = regex::Regex::new(r"\[\[(\d+)(?:\|([^\]]+))?\]\]").unwrap();

    let mut last_end = 0;
    let mut new_content = String::new();

    for cap in link_regex.captures_iter(&content) {
        let whole_match = cap.get(0).unwrap();
        let target_id: i32 = cap[1].parse().unwrap();

        // Get the path for this link
        let path = match get_note_path(state, &target_id, Some(&note_id)).await {
            Ok(p) => p,
            Err(_) => continue, // Skip this link if we can't get the path
        };

        // Use custom title if provided, otherwise use the path
        let display_text = cap.get(2).map(|m| m.as_str()).unwrap_or(&path);

        // Add the text between the last match and this one
        new_content.push_str(&content[last_end..whole_match.start()]);

        // Add the new formatted link
        new_content.push_str(&format!("[{}]({})", display_text, target_id));

        last_end = whole_match.end();
    }

    // Add any remaining content after the last match
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
    use crate::api::CreateNoteRequest;
    use crate::api::DieselError;
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
    async fn test_get_note_path() {
        let state = setup_test_state();
        use crate::api::create_note;
        use crate::api::CreateNoteRequest;

        // Create three notes with a hierarchy
        let note_a = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "".to_string(),
                content: "# Note A".to_string(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;

        let note_b = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "".to_string(),
                content: "# Note B".to_string(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;

        let note_c = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "".to_string(),
                content: "# Note C".to_string(),
            }),
        )
        .await
        .unwrap()
        .1
         .0;

        // Create hierarchy A -> B -> C
        attach_child_note(
            State(state.clone()),
            Json(AttachChildNoteRequest {
                parent_note_id: Some(note_a.id),
                child_note_id: note_b.id,
            }),
        )
        .await
        .unwrap();

        attach_child_note(
            State(state.clone()),
            Json(AttachChildNoteRequest {
                parent_note_id: Some(note_b.id),
                child_note_id: note_c.id,
            }),
        )
        .await
        .unwrap();

        // Test getting path for note C
        let path = get_note_path(&state, &note_c.id, None).await.unwrap();
        assert_eq!(path, "/ Note A / Note B / Note C");

        // Test getting path for note B
        let path = get_note_path(&state, &note_b.id, None).await.unwrap();
        assert_eq!(path, "/ Note A / Note B");

        // Test getting path for note A
        let path = get_note_path(&state, &note_a.id, None).await.unwrap();
        assert_eq!(path, "/ Note A");

        // Test getting path for note C From Note A
        let path = get_note_path(&state, &note_c.id, Some(&note_a.id))
            .await
            .unwrap();
        assert_eq!(path, "Note B / Note C");

        // Clean up
        let _cleanup = TestCleanup {
            pool: state.pool.as_ref().clone(),
            note_ids: vec![note_a.id, note_b.id, note_c.id],
        };
    }

    #[tokio::test]
    async fn test_get_note_content_and_replace_links() {
        let state = setup_test_state();
        use crate::api::{create_note, delete_note};

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

        // Create a note with various types of links
        let test_note = create_note(
            State(state.clone()),
            Json(CreateNoteRequest {
                title: "Test Note".to_string(),
                content: format!(
                    r"
# Test Note

Link to child: [[{child_id}]]

Link to unrelated: [[{unrelated_id}]]

Custom title link: [[{root_id}|Custom]]",
                    child_id = child_note.id,
                    unrelated_id = unrelated_note.id,
                    root_id = root_note.id
                ),
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
        let processed_content = get_note_content_and_replace_links(&state, test_note.id)
            .await
            .expect("Failed to process content");

        dbg!(&processed_content);

        // Verify the links are replaced correctly
        assert!(processed_content.contains(&format!("[/ Root / Child]({})", child_note.id)));
        assert!(processed_content.contains(&format!("[/ Unrelated]({})", unrelated_note.id)));
        assert!(processed_content.contains(&format!("[Custom]({})", root_note.id)));

        // Clean up
        let _cleanup = TestCleanup {
            pool: state.pool.as_ref().clone(),
            note_ids: vec![root_note.id, child_note.id, unrelated_note.id, test_note.id],
        };
    }
}
