use super::generics::attach_child;
use super::generics::is_circular_hierarchy;
use super::generics::{build_generic_tree, BasicTreeNode, HierarchyItem};
use crate::api::state::AppState;
use crate::tables::{Task, TaskHierarchy};
use axum::{extract::Path, extract::State, http::StatusCode, Json};
use diesel::result::QueryResult;
use diesel::{ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct AttachChildRequest {
    pub parent_task_id: Option<i32>,
    pub child_task_id: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskTreeNode {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub children: Vec<TaskTreeNode>,
}

impl HierarchyItem for TaskHierarchy {
    type Id = i32;

    fn get_parent_id(&self) -> Option<i32> {
        self.parent_task_id
    }

    fn get_child_id(&self) -> i32 {
        self.child_task_id
            .expect("child_task_id should not be None")
    }

    fn set_parent_id(&mut self, parent_id: Option<i32>) {
        self.parent_task_id = parent_id;
    }

    fn set_child_id(&mut self, child_id: i32) {
        self.child_task_id = Some(child_id);
    }

    fn find_by_child_id(conn: &mut PgConnection, child_id: Self::Id) -> QueryResult<Option<Self>> {
        use crate::schema::task_hierarchy::dsl::*;
        task_hierarchy
            .filter(child_task_id.eq(Some(child_id)))
            .first(conn)
            .optional()
    }

    fn insert_new(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::task_hierarchy::dsl::*;
        diesel::insert_into(task_hierarchy)
            .values(item)
            .execute(conn)
            .map(|_| ())
    }

    fn update_existing(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::task_hierarchy::dsl::*;
        diesel::update(task_hierarchy.find(item.id))
            .set((
                parent_task_id.eq(item.parent_task_id),
                child_task_id.eq(item.child_task_id),
            ))
            .execute(conn)
            .map(|_| ())
    }
}

fn convert_to_task_tree(basic_node: BasicTreeNode<Task>) -> TaskTreeNode {
    // For tasks, we'll use the associated note's content as title/description
    let (title, description) = if let Some(_note_id) = basic_node.data.note_id {
        // TODO: Fetch note details from database
        (format!("Task #{}", basic_node.data.id), None)
    } else {
        (format!("Task #{}", basic_node.data.id), None)
    };

    TaskTreeNode {
        id: basic_node.id,
        title,
        description,
        created_at: basic_node.data.created_at,
        modified_at: basic_node.data.modified_at,
        children: basic_node
            .children
            .into_iter()
            .map(convert_to_task_tree)
            .collect(),
    }
}

pub async fn get_task_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<TaskTreeNode>>, StatusCode> {
    use crate::schema::task_hierarchy::dsl::task_hierarchy;
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all tasks
    let all_tasks = Task::get_all(&mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all hierarchies
    let hierarchies: Vec<TaskHierarchy> = task_hierarchy
        .load::<TaskHierarchy>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prepare data for generic tree building
    let task_data: Vec<(i32, Task)> = all_tasks.into_iter().map(|task| (task.id, task)).collect();

    let hierarchy_tuples: Vec<(i32, i32)> = hierarchies
        .iter()
        .filter_map(|h| {
            h.child_task_id
                .zip(h.parent_task_id)
                .map(|(child, parent)| (child, parent))
        })
        .collect();

    // Build the basic tree and convert
    let tree = build_generic_tree(&task_data, &hierarchy_tuples)
        .into_iter()
        .map(convert_to_task_tree)
        .collect();

    Ok(Json(tree))
}

pub async fn detach_child_task(
    State(state): State<AppState>,
    Path(child_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use super::generics::detach_child;
    use crate::schema::task_hierarchy::dsl::{child_task_id, task_hierarchy};
    use diesel::prelude::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Define specific delete logic for the task hierarchy
    let delete_fn = |conn: &mut PgConnection, cid: i32| {
        diesel::delete(task_hierarchy.filter(child_task_id.eq(Some(cid)))).execute(conn)
    };

    // Call the generic detach_child function
    detach_child(delete_fn, child_id, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn attach_child_task(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Define the is_circular function specific to tasks
    let is_circular_fn = |conn: &mut PgConnection, child_id: i32, parent_id: Option<i32>| {
        is_circular_hierarchy(conn, child_id, parent_id)
    };

    // Create a TaskHierarchy item
    let item = TaskHierarchy {
        id: 0, // Assuming 'id' is auto-generated
        parent_task_id: payload.parent_task_id,
        child_task_id: Some(payload.child_task_id),
    };

    // Call the generic attach_child function with the specific implementation
    attach_child(is_circular_fn, item, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod task_hierarchy_tests {
    use super::*;
    use crate::tables::{NewTask, NewTaskHierarchy};
    use axum::extract::State;
    use axum::http::StatusCode;
    use diesel::result::Error as DieselError;
    use crate::api::tests::setup_test_state;

    #[tokio::test]
    async fn test_get_task_tree() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get connection from pool");

        conn.build_transaction()
            .read_write()
            .run::<_, DieselError, _>(|conn| {
                // Create test tasks
                use crate::schema::task_hierarchy::dsl::{
                    child_task_id, id as hierarchy_id, parent_task_id, task_hierarchy,
                };
                use crate::schema::tasks::dsl::{id as tasks_id, tasks};

                let root_task = diesel::insert_into(crate::schema::tasks::table)
                    .values(NewTask {
                        note_id: None,
                        status: "todo",
                        effort_estimate: None,
                        actual_effort: None,
                        deadline: None,
                        priority: Some(1),
                        created_at: Some(chrono::Utc::now().naive_utc()),
                        modified_at: Some(chrono::Utc::now().naive_utc()),
                        all_day: Some(false),
                        goal_relationship: None,
                    })
                    .get_result::<Task>(conn)?;

                let child_task = diesel::insert_into(crate::schema::tasks::table)
                    .values(NewTask {
                        note_id: None,
                        status: "todo",
                        effort_estimate: None,
                        actual_effort: None,
                        deadline: None,
                        priority: Some(2),
                        created_at: Some(chrono::Utc::now().naive_utc()),
                        modified_at: Some(chrono::Utc::now().naive_utc()),
                        all_day: Some(false),
                        goal_relationship: None,
                    })
                    .get_result::<Task>(conn)?;

                // Create hierarchy
                diesel::insert_into(task_hierarchy)
                    .values(NewTaskHierarchy {
                        parent_task_id: Some(root_task.id),
                        child_task_id: Some(child_task.id),
                    })
                    .execute(conn)?;

                // Get only the tasks created in this test
                let all_tasks = tasks
                    .filter(tasks_id.eq_any(vec![root_task.id, child_task.id]))
                    .load::<Task>(conn)?;

                // Get only the hierarchies created in this test
                let hierarchies = task_hierarchy
                    .filter(child_task_id.eq_any(vec![Some(child_task.id)]))
                    .load::<TaskHierarchy>(conn)?;

                // Build tree manually using the same logic as get_task_tree
                let task_data: Vec<(i32, Task)> =
                    all_tasks.into_iter().map(|task| (task.id, task)).collect();

                let hierarchy_tuples: Vec<(i32, i32)> = hierarchies
                    .iter()
                    .filter_map(|h| {
                        h.child_task_id
                            .zip(h.parent_task_id)
                            .map(|(child, parent)| (child, parent))
                    })
                    .collect();

                let tree = build_generic_tree(&task_data, &hierarchy_tuples)
                    .into_iter()
                    .map(convert_to_task_tree)
                    .collect::<Vec<_>>();

                // Verify tree structure
                assert_eq!(tree.len(), 1, "Expected one root task");
                assert_eq!(tree[0].title, format!("Task #{}", root_task.id));
                assert_eq!(tree[0].children.len(), 1, "Root should have one child");
                assert_eq!(
                    tree[0].children[0].title,
                    format!("Task #{}", child_task.id)
                );
                assert_eq!(
                    tree[0].children[0].children.len(),
                    0,
                    "Child should have no children"
                );

                Ok(())
            })
            .expect("Transaction failed");
    }

    #[tokio::test]
    async fn test_detach_child_task() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get connection from pool");

        conn.build_transaction()
            .read_write()
            .run::<_, DieselError, _>(|conn| {
                // Create test tasks
                let parent_task = diesel::insert_into(crate::schema::tasks::table)
                    .values(NewTask {
                        note_id: None,
                        status: "todo",
                        effort_estimate: None,
                        actual_effort: None,
                        deadline: None,
                        priority: Some(1),
                        created_at: Some(chrono::Utc::now().naive_utc()),
                        modified_at: Some(chrono::Utc::now().naive_utc()),
                        all_day: Some(false),
                        goal_relationship: None,
                    })
                    .get_result::<Task>(conn)?;

                let child_task = diesel::insert_into(crate::schema::tasks::table)
                    .values(NewTask {
                        note_id: None,
                        status: "todo",
                        effort_estimate: None,
                        actual_effort: None,
                        deadline: None,
                        priority: Some(2),
                        created_at: Some(chrono::Utc::now().naive_utc()),
                        modified_at: Some(chrono::Utc::now().naive_utc()),
                        all_day: Some(false),
                        goal_relationship: None,
                    })
                    .get_result::<Task>(conn)?;

                // Create hierarchy
                diesel::insert_into(crate::schema::task_hierarchy::table)
                    .values(NewTaskHierarchy {
                        parent_task_id: Some(parent_task.id),
                        child_task_id: Some(child_task.id),
                    })
                    .execute(conn)?;

                // Verify hierarchy exists
                let hierarchy_exists = crate::schema::task_hierarchy::dsl::task_hierarchy
                    .filter(
                        crate::schema::task_hierarchy::dsl::child_task_id.eq(Some(child_task.id)),
                    )
                    .first::<TaskHierarchy>(conn)
                    .optional()?
                    .is_some();

                assert!(hierarchy_exists, "Hierarchy should exist before detachment");

                // Call the detach_child_task function
                let response = tokio::runtime::Runtime::new()
                    .unwrap()
                    .block_on(async {
                        detach_child_task(State(state.clone()), axum::extract::Path(child_task.id))
                            .await
                    })
                    .expect("detach_child_task failed");

                assert_eq!(response, StatusCode::NO_CONTENT);

                // Verify hierarchy no longer exists
                let hierarchy_exists = crate::schema::task_hierarchy::dsl::task_hierarchy
                    .filter(
                        crate::schema::task_hierarchy::dsl::child_task_id.eq(Some(child_task.id)),
                    )
                    .first::<TaskHierarchy>(conn)
                    .optional()?
                    .is_some();

                assert!(
                    !hierarchy_exists,
                    "Hierarchy should not exist after detachment"
                );

                Ok(())
            })
            .expect("Transaction failed");
    }
}
