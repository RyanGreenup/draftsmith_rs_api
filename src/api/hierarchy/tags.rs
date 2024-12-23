use super::generics::{build_generic_tree, BasicTreeNode, HierarchyItem};
use crate::api::get_tags_notes;
use axum::extract::Query;
use crate::api::hierarchy::generics::{attach_child, is_circular_hierarchy, AttachChildRequest};
use crate::api::hierarchy::notes::{get_single_note_tree, NoteTreeNode};
use crate::api::state::AppState;
use crate::schema::tag_hierarchy;
use crate::schema::tags::dsl::{id as tag_id, tags};
use crate::tables::{NewTagHierarchy, Tag, TagHierarchy};
use axum::{debug_handler, extract::Path, extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use diesel::QueryResult;
use serde::{Deserialize, Serialize};

use crate::api::NoteMetadataResponse;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TagTreeNode {
    pub id: i32,
    pub name: String,
    pub children: Vec<TagTreeNode>,
    pub notes: Vec<NoteMetadataResponse>,
}

impl HierarchyItem for TagHierarchy {
    type Id = i32;

    fn get_parent_id(&self) -> Option<i32> {
        self.parent_tag_id
    }

    fn get_child_id(&self) -> i32 {
        self.child_tag_id.expect("child_tag_id should not be None")
    }

    fn set_parent_id(&mut self, parent_id: Option<i32>) {
        self.parent_tag_id = parent_id;
    }

    fn set_child_id(&mut self, child_id: i32) {
        self.child_tag_id = Some(child_id);
    }

    fn find_by_child_id(conn: &mut PgConnection, child_id: i32) -> QueryResult<Option<Self>> {
        use crate::schema::tag_hierarchy::dsl::*;

        tag_hierarchy
            .filter(child_tag_id.eq(child_id))
            .first::<TagHierarchy>(conn)
            .optional()
    }

    fn insert_new(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        let new_item = NewTagHierarchy {
            parent_tag_id: item.parent_tag_id,
            child_tag_id: item.child_tag_id,
        };

        diesel::insert_into(tag_hierarchy::table)
            .values(&new_item)
            .execute(conn)
            .map(|_| ())
    }

    fn update_existing(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::tag_hierarchy::dsl::*;

        diesel::update(tag_hierarchy.filter(child_tag_id.eq(item.get_child_id())))
            .set(parent_tag_id.eq(item.get_parent_id()))
            .execute(conn)
            .map(|_| ())
    }
}

#[debug_handler]
pub async fn detach_child_tag(
    State(state): State<AppState>,
    Path(child_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use super::generics::detach_child;
    use crate::schema::tag_hierarchy::dsl::{child_tag_id, tag_hierarchy};
    use crate::schema::tags::dsl::{id as tag_id, tags};
    use diesel::prelude::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if the child tag exists
    let child_exists = tags
        .filter(tag_id.eq(child_id))
        .first::<Tag>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if child_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Check if the hierarchy entry exists
    let hierarchy_exists = tag_hierarchy
        .filter(child_tag_id.eq(child_id))
        .first::<TagHierarchy>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if hierarchy_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Define specific delete logic for the tag hierarchy
    let delete_fn = |conn: &mut PgConnection, cid: i32| {
        diesel::delete(tag_hierarchy.filter(child_tag_id.eq(cid))).execute(conn)
    };

    // Call the generic detach_child function
    detach_child(delete_fn, child_id, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn attach_child_tag(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if the parent tag exists (if parent_id is provided)
    if let Some(parent_id) = payload.parent_id {
        let parent_exists = tags
            .filter(tag_id.eq(parent_id))
            .first::<Tag>(&mut conn)
            .optional()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        if parent_exists.is_none() {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // Check if the child tag exists
    let child_exists = tags
        .filter(tag_id.eq(payload.child_id))
        .first::<Tag>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if child_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Define a function to get the parent ID of a given child ID from tag_hierarchy
    let get_parent_fn = |conn: &mut PgConnection, child_id: i32| {
        use crate::schema::tag_hierarchy::dsl::*;
        tag_hierarchy
            .filter(child_tag_id.eq(child_id))
            .select(parent_tag_id)
            .first::<Option<i32>>(conn)
            .optional()
            .map(|opt| opt.flatten())
    };

    // Check for circular reference before proceeding
    if let Some(parent_id) = payload.parent_id {
        if is_circular_hierarchy(&mut conn, payload.child_id, Some(parent_id), get_parent_fn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Create a TagHierarchy item
    let item = TagHierarchy {
        id: 0, // Assuming 'id' is auto-generated by the database
        parent_tag_id: payload.parent_id,
        child_tag_id: Some(payload.child_id),
    };

    // Call the generic attach_child function
    attach_child(
        |conn, child_id, parent_id| is_circular_hierarchy(conn, child_id, parent_id, get_parent_fn),
        item,
        &mut conn,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize)]
pub struct GetTagTreeParams {
    #[serde(default = "default_include_subpages")]
    pub include_subpages: bool,
    #[serde(default = "default_exclude_content")]
    pub exclude_content: bool,
}

fn default_include_subpages() -> bool {
    false
}

fn default_exclude_content() -> bool {
    false
}

pub async fn get_tag_tree(
    State(state): State<AppState>,
    Query(params): Query<GetTagTreeParams>,
) -> Result<Json<Vec<TagTreeNode>>, StatusCode> {
    use crate::schema::tag_hierarchy::dsl::tag_hierarchy;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    use crate::schema::tags::dsl::*;
    let all_tags = tags
        .load::<Tag>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all hierarchies
    let hierarchies: Vec<TagHierarchy> = tag_hierarchy
        .load::<TagHierarchy>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prepare data for generic tree building
    let tag_data: Vec<(i32, Tag)> = all_tags.into_iter().map(|tag| (tag.id, tag)).collect();

    let hierarchy_tuples: Vec<(i32, i32)> = hierarchies
        .iter()
        .map(|h| {
            (
                h.child_tag_id.expect("child_tag_id should not be None"),
                h.parent_tag_id.unwrap_or(0),
            )
        })
        .collect();

    // Build the basic tree
    let basic_tree = build_generic_tree(&tag_data, &hierarchy_tuples);

    // Convert BasicTreeNode to TagTreeNode
    async fn convert_to_tag_tree(
        basic_node: BasicTreeNode<Tag>,
        state: &AppState,
        include_subpages: bool,
        exclude_content: bool,
    ) -> TagTreeNode {
        let mut notes = get_tags_notes(State(state.clone()), vec![basic_node.id])
            .await
            .unwrap_or_default()
            .0
            .remove(&basic_node.id)
            .unwrap_or_default();

        if include_subpages {
            let mut all_descendants = Vec::new();
            for note in &notes {
                if let Ok(note_tree) = get_single_note_tree(&state, note.id, exclude_content).await {
                    // Don't include the root note since it's already in `notes`
                    for child in &note_tree {
                        all_descendants.extend(collect_all_notes(&[child.clone()]));
                    }
                }
            }
            // Deduplicate notes by ID before extending
            all_descendants.retain(|descendant| !notes.iter().any(|n| n.id == descendant.id));
            notes.extend(all_descendants);
        }

        TagTreeNode {
            id: basic_node.id,
            name: basic_node.data.name,
            children: futures::future::join_all(
                basic_node
                    .children
                    .into_iter()
                    .map(|child| convert_to_tag_tree(child, state, include_subpages, exclude_content)),
            )
            .await,
            notes,
        }
    }

    // Helper function to recursively collect all notes from a tree
    fn collect_all_notes(tree: &[NoteTreeNode]) -> Vec<NoteMetadataResponse> {
        let mut notes = Vec::new();
        for node in tree {
            notes.push(NoteMetadataResponse {
                id: node.id,
                title: node.title.clone().unwrap_or_default(),
                created_at: node.created_at,
                modified_at: node.modified_at,
            });
            notes.extend(collect_all_notes(&node.children));
        }
        notes
    }

    let tree = futures::future::join_all(
        basic_tree
            .into_iter()
            .map(|node| convert_to_tag_tree(node, &state, params.include_subpages, params.exclude_content)),
    )
    .await;

    Ok(Json(tree))
}

#[derive(Debug, Serialize)]
pub struct HierarchyMapping {
    pub parent_id: Option<i32>,
    pub child_id: i32,
}

impl TagHierarchy {
    pub fn get_hierarchy_mappings(conn: &mut PgConnection) -> QueryResult<Vec<HierarchyMapping>> {
        use crate::schema::tag_hierarchy::dsl::*;

        tag_hierarchy
            .select((parent_tag_id, child_tag_id))
            .load::<(Option<i32>, Option<i32>)>(conn)
            .map(|results| {
                results
                    .into_iter()
                    .filter_map(|(p, c)| {
                        c.map(|child| HierarchyMapping {
                            parent_id: p,
                            child_id: child,
                        })
                    })
                    .collect()
            })
    }
}

pub async fn get_hierarchy_mappings(
    State(state): State<AppState>,
) -> Result<Json<Vec<HierarchyMapping>>, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response = TagHierarchy::get_hierarchy_mappings(&mut conn).map_err(|e| {
        tracing::error!("Error getting hierarchy mappings: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(response))
}

use lazy_static::lazy_static;
use std::sync::Mutex;

lazy_static! {
    static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::notes;
    use crate::schema::note_hierarchy;
    use crate::schema::note_tags;
    use crate::tables::{NewNote, NewNoteHierarchy, NewNoteTag, NoteWithoutFts};
    use diesel::associations::HasTable;
    use super::*;
    use crate::api::tests::setup_test_state;
    use crate::tables::{NewTag, NewTagHierarchy};
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::Json;

    #[tokio::test]
    async fn test_attach_child_tag_detects_cycle() {
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Import necessary items
        use crate::schema::tags::dsl::tags;

        // Create test tags within a transaction
        let (tag1_id, tag2_id) = conn
            .build_transaction()
            .read_write()
            .run::<_, diesel::result::Error, _>(|conn| {
                // Create two tags
                let tag1 = diesel::insert_into(tags)
                    .values(NewTag { name: "Tag1" })
                    .get_result::<Tag>(conn)?;
                let tag2 = diesel::insert_into(tags)
                    .values(NewTag { name: "Tag2" })
                    .get_result::<Tag>(conn)?;
                Ok((tag1.id, tag2.id))
            })
            .expect("Transaction failed");

        // Attach tag2 as a child of tag1
        let payload = AttachChildRequest {
            parent_id: Some(tag1_id),
            child_id: tag2_id,
        };
        let status = attach_child_tag(State(state.clone()), Json(payload))
            .await
            .expect("Failed to attach child tag");
        assert_eq!(status, StatusCode::OK);

        // Attempt to create a cycle by attaching tag1 as a child of tag2
        let cyclic_payload = AttachChildRequest {
            parent_id: Some(tag2_id),
            child_id: tag1_id,
        };
        let result = attach_child_tag(State(state), Json(cyclic_payload)).await;
        assert_eq!(result, Err(StatusCode::BAD_REQUEST));
    }

    #[tokio::test]
    async fn test_detach_child_tag() {
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Import necessary items
        use crate::schema::tag_hierarchy::dsl::{child_tag_id, tag_hierarchy};
        use crate::schema::tags::dsl::tags;

        // Declare variables to hold the tag IDs
        let mut parent_tag_id: Option<i32> = None;
        let mut child_tag_id_value: Option<i32> = None;

        // Create test tags and hierarchy within a transaction
        conn.build_transaction()
            .read_write()
            .run::<_, diesel::result::Error, _>(|conn| {
                // Create parent tag
                let parent_tag = diesel::insert_into(tags::table())
                    .values(NewTag { name: "parent_tag" })
                    .get_result::<Tag>(conn)?;

                // Create child tag
                let child_tag = diesel::insert_into(tags)
                    .values(NewTag { name: "child_tag" })
                    .get_result::<Tag>(conn)?;

                // Store the tag IDs
                parent_tag_id = Some(parent_tag.id);
                child_tag_id_value = Some(child_tag.id);

                // Create hierarchy link between parent and child
                diesel::insert_into(tag_hierarchy)
                    .values(NewTagHierarchy {
                        parent_tag_id: Some(parent_tag.id),
                        child_tag_id: Some(child_tag.id),
                    })
                    .execute(conn)?;

                Ok(())
            })
            .expect("Transaction failed");

        // Unwrap the tag IDs
        let child_tag_id_value = child_tag_id_value.expect("Failed to retrieve child_tag_id");

        // Call detach_child_tag to detach the child
        let status = detach_child_tag(State(state.clone()), Path(child_tag_id_value))
            .await
            .expect("Failed to detach child tag");

        assert_eq!(status, StatusCode::NO_CONTENT);

        // Verify that the hierarchy link has been removed
        let hierarchy_exists = tag_hierarchy
            .filter(child_tag_id.eq(child_tag_id_value))
            .first::<TagHierarchy>(&mut conn)
            .optional()
            .expect("Failed to query tag_hierarchy")
            .is_some();

        assert!(
            !hierarchy_exists,
            "Hierarchy link should be deleted after detaching child"
        );
    }

    #[tokio::test]
    async fn test_attach_child_tag() {
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Import necessary items
        use crate::schema::tag_hierarchy::dsl::{child_tag_id, parent_tag_id, tag_hierarchy};
        use crate::schema::tags::dsl::tags;
        use crate::tables::{NewTag, Tag};

        // Create test tags within a transaction
        let (parent_tag_id_value, child_tag_id_value) = conn
            .build_transaction()
            .read_write()
            .run::<_, diesel::result::Error, _>(|conn| {
                // Create parent tag
                let parent_tag = diesel::insert_into(tags)
                    .values(NewTag { name: "parent_tag" })
                    .get_result::<Tag>(conn)?;

                // Create child tag
                let child_tag = diesel::insert_into(tags)
                    .values(NewTag { name: "child_tag" })
                    .get_result::<Tag>(conn)?;

                Ok((parent_tag.id, child_tag.id))
            })
            .expect("Transaction failed");

        // Prepare the request payload
        let payload = AttachChildRequest {
            parent_id: Some(parent_tag_id_value),
            child_id: child_tag_id_value,
        };

        // Call attach_child_tag
        let status = attach_child_tag(State(state.clone()), Json(payload))
            .await
            .expect("Failed to attach child tag");

        assert_eq!(status, StatusCode::OK);

        // Verify that the hierarchy link has been created
        let hierarchy_exists = tag_hierarchy
            .filter(child_tag_id.eq(child_tag_id_value))
            .filter(parent_tag_id.eq(Some(parent_tag_id_value)))
            .first::<TagHierarchy>(&mut conn)
            .optional()
            .expect("Failed to query tag_hierarchy")
            .is_some();

        assert!(
            hierarchy_exists,
            "Hierarchy link should exist after attaching child"
        );
    }

    #[tokio::test]
    async fn test_get_tag_tree_with_subpages() {
        let _lock = TEST_MUTEX.lock().unwrap();
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Get initial tag count
        let initial_tags: Vec<TagTreeNode> = get_tag_tree(
            State(state.clone()),
            Query(GetTagTreeParams {
                include_subpages: false,
                exclude_content: false,
            })
        ).await.unwrap().0;
        
        let initial_tag_count = initial_tags.len();

        // Create test data and store IDs for cleanup
        let (parent_tag_id, parent_note_id, child_note_id) = conn
            .build_transaction()
            .read_write()
            .run::<_, diesel::result::Error, _>(|conn| {
                // Create a tag
                let parent_tag = diesel::insert_into(tags::table())
                    .values(NewTag { name: "parent_tag" })
                    .get_result::<Tag>(conn)?;

                // Create a note
                let parent_note = diesel::insert_into(notes::table)
                    .values(NewNote {
                        title: "Parent Note",
                        content: "Parent content",
                        created_at: Some(chrono::Utc::now().naive_utc()),
                        modified_at: Some(chrono::Utc::now().naive_utc()),
                    })
                    .returning((
                        notes::id,
                        notes::title,
                        notes::content,
                        notes::created_at,
                        notes::modified_at,
                    ))
                    .get_result::<NoteWithoutFts>(conn)?;

                // Create a subpage
                let child_note = diesel::insert_into(notes::table)
                    .values(NewNote {
                        title: "Child Note",
                        content: "Child content",
                        created_at: Some(chrono::Utc::now().naive_utc()),
                        modified_at: Some(chrono::Utc::now().naive_utc()),
                    })
                    .returning((
                        notes::id,
                        notes::title,
                        notes::content,
                        notes::created_at,
                        notes::modified_at,
                    ))
                    .get_result::<NoteWithoutFts>(conn)?;

                // Link note to tag
                diesel::insert_into(note_tags::table)
                    .values(NewNoteTag {
                        note_id: parent_note.id,
                        tag_id: parent_tag.id,
                    })
                    .execute(conn)?;

                // Create note hierarchy
                diesel::insert_into(note_hierarchy::table)
                    .values(NewNoteHierarchy {
                        parent_note_id: Some(parent_note.id),
                        child_note_id: Some(child_note.id),
                    })
                    .execute(conn)?;

                Ok((parent_tag.id, parent_note.id, child_note.id))
            })
            .expect("Transaction failed");

        // Get final tag count
        let final_tags: Vec<TagTreeNode> = get_tag_tree(
            State(state.clone()),
            Query(GetTagTreeParams {
                include_subpages: false,
                exclude_content: false,
            })
        ).await.unwrap().0;

        // Verify we only added one new tag
        assert_eq!(
            final_tags.len() - initial_tag_count,
            1,
            "Expected to add exactly one new tag. Initial count: {}, Final count: {}",
            initial_tag_count,
            final_tags.len()
        );

        // Find the new tag
        let new_tag = final_tags
            .iter()
            .find(|t| !initial_tags.iter().any(|it| it.id == t.id))
            .expect("Could not find the newly added tag");

        // Verify the new tag has one note
        assert_eq!(
            new_tag.notes.len(),
            1,
            "New tag should have exactly one note"
        );

        // Test with subpages
        let final_tags_with_subpages = get_tag_tree(
            State(state.clone()),
            Query(GetTagTreeParams {
                include_subpages: true,
                exclude_content: false,
            })
        ).await.unwrap().0;

        // Find the new tag again
        let new_tag_with_subpages = final_tags_with_subpages
            .iter()
            .find(|t| !initial_tags.iter().any(|it| it.id == t.id))
            .expect("Could not find the newly added tag");

        // Verify it now has both parent and child notes
        assert_eq!(
            new_tag_with_subpages.notes.len(),
            2,
            "Should have both parent and child notes"
        );

        // Clean up is handled by test database teardown
    }

    #[tokio::test]
    async fn test_get_tag_tree() {
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Import only necessary items and alias conflicting names
        use crate::schema::tag_hierarchy::dsl::tag_hierarchy;
        use crate::schema::tags::dsl::tags;

        // Declare variables to hold the tag IDs
        let mut root_tag_id: Option<i32> = None;
        let mut child1_tag_id: Option<i32> = None;
        let mut child2_tag_id: Option<i32> = None;

        conn.build_transaction()
            .read_write()
            .run::<_, diesel::result::Error, _>(|conn| {
                // Create test tags
                let root_tag = diesel::insert_into(tags)
                    .values(NewTag {
                        name: "test_root_tag",
                    })
                    .get_result::<Tag>(conn)
                    .expect("Failed to create root tag");

                let child1_tag = diesel::insert_into(tags)
                    .values(NewTag {
                        name: "test_child1_tag",
                    })
                    .get_result::<Tag>(conn)
                    .expect("Failed to create child1 tag");

                let child2_tag = diesel::insert_into(tags)
                    .values(NewTag {
                        name: "test_child2_tag",
                    })
                    .get_result::<Tag>(conn)
                    .expect("Failed to create child2 tag");

                // Store the IDs
                root_tag_id = Some(root_tag.id);
                child1_tag_id = Some(child1_tag.id);
                child2_tag_id = Some(child2_tag.id);

                // Create hierarchy: root -> child1 -> child2
                diesel::insert_into(tag_hierarchy)
                    .values(NewTagHierarchy {
                        child_tag_id: Some(child1_tag.id),
                        parent_tag_id: Some(root_tag.id),
                    })
                    .execute(conn)
                    .expect("Failed to create first hierarchy link");

                diesel::insert_into(tag_hierarchy)
                    .values(NewTagHierarchy {
                        child_tag_id: Some(child2_tag.id),
                        parent_tag_id: Some(child1_tag.id),
                    })
                    .execute(conn)
                    .expect("Failed to create second hierarchy link");

                Ok(())
            })
            .expect("Transaction failed");

        // Unwrap the tag IDs
        let root_tag_id = root_tag_id.expect("Failed to retrieve root_tag_id");
        let child1_tag_id = child1_tag_id.expect("Failed to retrieve child1_tag_id");
        let child2_tag_id = child2_tag_id.expect("Failed to retrieve child2_tag_id");

        // Call get_tag_tree
        let response = get_tag_tree(
            State(state),
            Query(GetTagTreeParams {
                include_subpages: false,
                exclude_content: false,
            })
        ).await.unwrap();
        let tree = response.0;

        // Find our test root tag in the tree by ID
        let test_tree: Vec<_> = tree
            .into_iter()
            .filter(|node| node.id == root_tag_id)
            .collect();

        assert_eq!(test_tree.len(), 1, "Should find exactly one test root tag");
        let root = &test_tree[0];
        assert_eq!(root.children.len(), 1);

        let child1 = &root.children[0];
        assert_eq!(child1.id, child1_tag_id);
        assert_eq!(child1.children.len(), 1);

        let child2 = &child1.children[0];
        assert_eq!(child2.id, child2_tag_id);
        assert_eq!(child2.children.len(), 0);
    }
}
