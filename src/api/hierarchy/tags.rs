use super::generics::{build_generic_tree, BasicTreeNode, HierarchyItem};
use crate::api::state::AppState;
use crate::schema::tag_hierarchy;
use crate::tables::{NewTagHierarchy, Tag, TagHierarchy};
use axum::{debug_handler, extract::State, http::StatusCode, Json};
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
pub async fn get_tag_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<TagTreeNode>>, StatusCode> {
    use crate::schema::tag_hierarchy::dsl::tag_hierarchy;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all tags
    let all_tags = Tag::get_all(&mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all hierarchies
    let hierarchies: Vec<TagHierarchy> = tag_hierarchy
        .load::<TagHierarchy>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prepare data for generic tree building
    let tag_data: Vec<(i32, Tag)> = all_tags.into_iter().map(|tag| (tag.id, tag)).collect();

    let hierarchy_tuples: Vec<(i32, i32)> = hierarchies
        .iter()
        .map(|h| (h.child_tag_id.expect("child_tag_id should not be None"), h.parent_tag_id.unwrap_or(0)))
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
    use crate::tables::NewTag;

    #[tokio::test]
    async fn test_get_tag_tree() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get database connection");


        // Create test tags
        let root_tag = diesel::insert_into(crate::schema::tags::table)
            .values(NewTag {
                name: "root_tag",
            })
            .get_result::<Tag>(&mut conn)
            .expect("Failed to create root tag");

        let child1_tag = diesel::insert_into(crate::schema::tags::table)
            .values(NewTag {
                name: "child1_tag",
            })
            .get_result::<Tag>(&mut conn)
            .expect("Failed to create child1 tag");

        let child2_tag = diesel::insert_into(crate::schema::tags::table)
            .values(NewTag {
                name: "child2_tag",
            })
            .get_result::<Tag>(&mut conn)
            .expect("Failed to create child2 tag");

        // Create hierarchy: root -> child1 -> child2
        diesel::insert_into(crate::schema::tag_hierarchy::table)
            .values(NewTagHierarchy {
                child_tag_id: Some(child1_tag.id),
                parent_tag_id: Some(root_tag.id),
            })
            .execute(&mut conn)
            .expect("Failed to create first hierarchy link");

        diesel::insert_into(crate::schema::tag_hierarchy::table)
            .values(NewTagHierarchy {
                child_tag_id: Some(child2_tag.id),
                parent_tag_id: Some(child1_tag.id),
            })
            .execute(&mut conn)
            .expect("Failed to create second hierarchy link");

        // Call get_tag_tree
        let response = get_tag_tree(State(state.clone())).await.unwrap();
        let tree = response.0;

        // Clean up test data
        diesel::delete(crate::schema::tag_hierarchy::table)
            .execute(&mut conn)
            .expect("Failed to clean up tag hierarchies");
        diesel::delete(crate::schema::tags::table)
            .execute(&mut conn)
            .expect("Failed to clean up tags");

        // Verify the tree structure
        assert_eq!(tree.len(), 1); // Should have one root node
        let root = &tree[0];
        assert_eq!(root.name, "root_tag");
        assert_eq!(root.children.len(), 1);

        let child1 = &root.children[0];
        assert_eq!(child1.name, "child1_tag");
        assert_eq!(child1.children.len(), 1);

        let child2 = &child1.children[0];
        assert_eq!(child2.name, "child2_tag");
        assert_eq!(child2.children.len(), 0);
    }
}