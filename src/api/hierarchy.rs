use crate::api::state::AppState;
use crate::api::AttachChildRequest;

#[derive(Debug)]
pub struct BasicTreeNode<T> {
    pub id: i32,
    pub data: T,
    pub children: Vec<BasicTreeNode<T>>,
}

// Generic function that builds a basic tree structure
pub fn build_generic_tree<T>(
    items: &[(i32, T)],
    hierarchies: &[(i32, i32)], // (child_id, parent_id)
) -> Vec<BasicTreeNode<T>> 
where
    T: Clone,
{
    // Create a map of parent_id to children
    let mut parent_to_children: HashMap<Option<i32>, Vec<i32>> = HashMap::new();
    
    // Track which items are children
    let mut child_items: HashSet<i32> = HashSet::new();

    // Build the parent-to-children mapping
    for (child_id, parent_id) in hierarchies {
        parent_to_children
            .entry(Some(*parent_id))
            .or_default()
            .push(*child_id);
        child_items.insert(*child_id);
    }

    // Create a map of item id to data for easy lookup
    let items_map: HashMap<_, _> = items.iter().map(|(item_id, data)| (*item_id, data)).collect();

    // Function to recursively build the tree
    fn build_subtree<T: Clone>(
        item_id: i32,
        items_map: &HashMap<i32, &T>,
        parent_to_children: &HashMap<Option<i32>, Vec<i32>>,
    ) -> BasicTreeNode<T> {
        let children = parent_to_children
            .get(&Some(item_id))
            .map(|children| {
                children
                    .iter()
                    .map(|child_id| build_subtree(*child_id, items_map, parent_to_children))
                    .collect()
            })
            .unwrap_or_default();

        BasicTreeNode {
            id: item_id,
            data: (*items_map.get(&item_id).unwrap()).clone(),
            children,
        }
    }

    // Build trees starting from root items (items that aren't children)
    let mut tree: Vec<BasicTreeNode<T>> = items
        .iter()
        .filter(|(item_id, _)| !child_items.contains(item_id))
        .map(|(item_id, _)| build_subtree(*item_id, &items_map, &parent_to_children))
        .collect();

    // Sort the tree by item ID for consistent ordering
    tree.sort_by_key(|node| node.id);

    tree
}
use crate::api::Path;
use crate::schema::note_hierarchy::dsl::*;
use crate::tables::{NewNote, NewNoteHierarchy, NoteHierarchy, NoteWithoutFts};
use axum::{extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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

// Modify Note Hierarchy

pub async fn attach_child_note(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildRequest>,
) -> Result<StatusCode, StatusCode> {
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

pub async fn detach_child_note(
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

pub async fn get_note_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteTreeNode>>, StatusCode> {
    use crate::schema::note_hierarchy::dsl::note_hierarchy;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all notes
    let all_notes = NoteWithoutFts::get_all(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all hierarchies
    let hierarchies: Vec<NoteHierarchy> = note_hierarchy
        .load::<NoteHierarchy>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prepare data for generic tree building
    let note_data: Vec<(i32, NoteWithoutFts)> = all_notes
        .into_iter()
        .map(|note| (note.id, note))
        .collect();

    let hierarchy_tuples: Vec<(i32, i32)> = hierarchies
        .iter()
        .filter_map(|h| {
            h.child_note_id
                .zip(h.parent_note_id)
                .map(|(child, parent)| (child, parent))
        })
        .collect();

    // Build the basic tree
    let basic_tree = build_generic_tree(&note_data, &hierarchy_tuples);

    // Convert BasicTreeNode to NoteTreeNode
    fn convert_to_note_tree(basic_node: BasicTreeNode<NoteWithoutFts>) -> NoteTreeNode {
        NoteTreeNode {
            id: basic_node.id,
            title: Some(basic_node.data.title),
            content: Some(basic_node.data.content),
            created_at: basic_node.data.created_at,
            modified_at: basic_node.data.modified_at,
            hierarchy_type: None, // This will be handled separately if needed
            children: basic_node.children.into_iter().map(convert_to_note_tree).collect(),
        }
    }

    let tree = basic_tree.into_iter().map(convert_to_note_tree).collect();

    Ok(Json(tree))
}

// Handler for the PUT /notes/tree endpoint
pub async fn update_note_tree(
    State(state): State<AppState>,
    Json(note_tree): Json<NoteTreeNode>,
) -> Result<StatusCode, StatusCode> {
    update_database_from_notetreenode(State(state), Json(note_tree)).await
}

pub async fn update_database_from_notetreenode(
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
                Ok(other_id) => {
                    eprintln!("Inserted new note with id: {}", other_id);
                    other_id
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

#[cfg(test)]
mod note_hierarchy_tests {
    use super::*;
    use crate::api::tests::{setup_test_state, TestCleanup};
    use crate::tables::Note;
    use axum::extract::State;
    use axum::Json;

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
}

// Modify Tag Hierarchy

// Modify Task Hierarchy
