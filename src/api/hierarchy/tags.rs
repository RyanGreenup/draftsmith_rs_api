use super::generics::{build_generic_tree, BasicTreeNode, HierarchyItem};
use crate::api::hierarchy::generics::{attach_child, is_circular_hierarchy, AttachChildRequest};
use crate::api::state::AppState;
use crate::schema::tag_hierarchy;
use crate::schema::tags::dsl::{id as tag_id, tags};
use crate::tables::{NewTagHierarchy, Tag, TagHierarchy};
use axum::{debug_handler, extract::Path, extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TagTreeNode {
    pub id: i32,
    pub name: String,
    pub children: Vec<TagTreeNode>,
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

pub async fn get_tag_tree(
    State(state): State<AppState>,
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
    fn convert_to_tag_tree(basic_node: BasicTreeNode<Tag>) -> TagTreeNode {
        TagTreeNode {
            id: basic_node.id,
            name: basic_node.data.name,
            children: basic_node
                .children
                .into_iter()
                .map(convert_to_tag_tree)
                .collect(),
        }
    }

    let tree = basic_tree.into_iter().map(convert_to_tag_tree).collect();

    Ok(Json(tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::tests::setup_test_state;
    use crate::tables::{NewTag, NewTagHierarchy};
    use axum::extract::State;
    use axum::Json;
    use axum::http::StatusCode;
    use diesel::prelude::*;

    #[tokio::test]
    async fn test_attach_child_tag_detects_cycle() {
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Import necessary items
        use crate::schema::tag_hierarchy::dsl::tag_hierarchy;
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
                let parent_tag = diesel::insert_into(tags)
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
    async fn test_get_tag_tree() {
        let state = setup_test_state();
        let mut conn = state.pool.get().expect("Failed to get database connection");

        // Import only necessary items and alias conflicting names
        use crate::schema::tag_hierarchy::dsl::{
            child_tag_id, id as hierarchy_id, parent_tag_id, tag_hierarchy,
        };
        use crate::schema::tags::dsl::{id as tags_id, tags};

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
        let response = get_tag_tree(State(state)).await.unwrap();
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
