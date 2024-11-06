use super::generics::{attach_child, is_circular_hierarchy};
use super::generics::{build_generic_tree, BasicTreeNode, HierarchyItem};
use crate::api::state::AppState;
use crate::schema::tasks::dsl::{id as task_id, tasks};
use crate::tables::{NewTaskHierarchy, Task, TaskHierarchy};
use axum::{extract::Path, extract::State, http::StatusCode, Json};
use diesel::result::QueryResult;
use diesel::{ExpressionMethods, OptionalExtension, PgConnection, QueryDsl, RunQueryDsl};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct AttachChildRequest {
    pub parent_task_id: Option<i32>,
    pub child_task_id: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HierarchyMapping {
    pub parent_id: Option<i32>,
    pub child_id: i32,
}

impl TaskHierarchy {
    pub fn get_hierarchy_mappings(conn: &mut PgConnection) -> QueryResult<Vec<HierarchyMapping>> {
        use crate::schema::task_hierarchy::dsl::*;

        task_hierarchy
            .select((parent_task_id, child_task_id))
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

    let response = TaskHierarchy::get_hierarchy_mappings(&mut conn).map_err(|e| {
        tracing::error!("Error getting hierarchy mappings: {:?}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(Json(response))
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskTreeNode {
    pub id: i32,
    pub note_id: Option<i32>,
    pub status: String,
    pub effort_estimate: Option<bigdecimal::BigDecimal>,
    pub actual_effort: Option<bigdecimal::BigDecimal>,
    pub deadline: Option<chrono::NaiveDateTime>,
    pub priority: Option<i32>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<i32>,
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
        let new_item = NewTaskHierarchy {
            parent_task_id: item.parent_task_id,
            child_task_id: item.child_task_id,
        };
        diesel::insert_into(task_hierarchy)
            .values(&new_item)
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
    TaskTreeNode {
        id: basic_node.id,
        note_id: basic_node.data.note_id,
        status: basic_node.data.status,
        effort_estimate: basic_node.data.effort_estimate,
        actual_effort: basic_node.data.actual_effort,
        deadline: basic_node.data.deadline,
        priority: basic_node.data.priority,
        created_at: basic_node.data.created_at,
        modified_at: basic_node.data.modified_at,
        all_day: basic_node.data.all_day,
        goal_relationship: basic_node.data.goal_relationship,
        children: basic_node
            .children
            .into_iter()
            .map(convert_to_task_tree)
            .collect(),
    }
}

pub async fn get_task_tree(
    State(state): State<AppState>,
) -> Result<(StatusCode, Json<Vec<TaskTreeNode>>), StatusCode> {
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

    Ok((StatusCode::OK, Json(tree)))
}

#[axum::debug_handler]
pub async fn detach_child_task(
    State(state): State<AppState>,
    Path(child_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use super::generics::detach_child;
    use crate::schema::task_hierarchy::dsl::{child_task_id, task_hierarchy};
    use crate::schema::tasks::dsl::{id as task_id, tasks};
    use diesel::prelude::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if the child task exists
    let child_exists = tasks
        .filter(task_id.eq(child_id))
        .first::<Task>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if child_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Check if the hierarchy entry exists
    let hierarchy_exists = task_hierarchy
        .filter(child_task_id.eq(Some(child_id)))
        .first::<TaskHierarchy>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if hierarchy_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Define specific delete logic for the task hierarchy
    let delete_fn = |conn: &mut PgConnection, cid: i32| {
        diesel::delete(task_hierarchy.filter(child_task_id.eq(Some(cid)))).execute(conn)
    };

    // Call the generic detach_child function
    detach_child(delete_fn, child_id, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[axum::debug_handler]
pub async fn attach_child_task(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if the parent task exists (if provided)
    if let Some(parent_id) = payload.parent_task_id {
        let parent_exists = tasks
            .filter(task_id.eq(parent_id))
            .first::<Task>(&mut conn)
            .optional()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        if parent_exists.is_none() {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // Check if the child task exists
    let child_exists = tasks
        .filter(task_id.eq(payload.child_task_id))
        .first::<Task>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if child_exists.is_none() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Define function to get parent ID from task hierarchy
    let get_parent_fn = |conn: &mut PgConnection, child_id: i32| -> QueryResult<Option<i32>> {
        use crate::schema::task_hierarchy::dsl::*;
        task_hierarchy
            .filter(child_task_id.eq(Some(child_id)))
            .select(parent_task_id)
            .first::<Option<i32>>(conn)
            .optional()
            .map(|opt| opt.flatten())
    };

    // Check for circular reference before proceeding
    if let Some(parent_id) = payload.parent_task_id {
        if is_circular_hierarchy(
            &mut conn,
            payload.child_task_id,
            Some(parent_id),
            get_parent_fn,
        )
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            return Err(StatusCode::BAD_REQUEST);
        }
    }

    // Create a TaskHierarchy item
    let item = TaskHierarchy {
        id: 0, // Assuming 'id' is auto-generated
        parent_task_id: payload.parent_task_id,
        child_task_id: Some(payload.child_task_id),
    };

    // Define the is_circular function specific to tasks
    let is_circular_fn = |conn: &mut PgConnection, child_id: i32, parent_id: Option<i32>| {
        is_circular_hierarchy(conn, child_id, parent_id, get_parent_fn)
    };

    // Call the generic attach_child function with the specific implementation
    attach_child(is_circular_fn, item, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod task_hierarchy_tests {
    use super::*;
    use crate::api::tests::setup_test_state;
    use crate::tables::{NewTask, NewTaskHierarchy};
    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::Json;
    use diesel::result::Error as DieselError;

    #[tokio::test]
    async fn test_attach_child_task_detects_cycle() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get connection from pool");

        use crate::schema::tasks::dsl::{id as task_id, tasks};
        use diesel::prelude::*;

        // Create two tasks
        let parent_task = diesel::insert_into(tasks)
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
            .get_result::<Task>(&mut conn)
            .expect("Failed to create parent task");

        let child_task = diesel::insert_into(tasks)
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
            .get_result::<Task>(&mut conn)
            .expect("Failed to create child task");

        // Attach child_task to parent_task
        let payload = AttachChildRequest {
            parent_task_id: Some(parent_task.id),
            child_task_id: child_task.id,
        };
        let status = attach_child_task(State(state.clone()), Json(payload))
            .await
            .expect("Failed to attach child task");
        assert_eq!(status, StatusCode::OK);

        // Attempt to create a cycle by attaching parent_task as a child of child_task
        let cyclic_payload = AttachChildRequest {
            parent_task_id: Some(child_task.id),
            child_task_id: parent_task.id,
        };
        let result = attach_child_task(State(state), Json(cyclic_payload)).await;
        assert_eq!(result, Err(StatusCode::BAD_REQUEST));
    }

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
                assert_eq!(tree[0].id, root_task.id);
                assert_eq!(tree[0].children.len(), 1, "Root should have one child");
                assert_eq!(tree[0].children[0].id, child_task.id);
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
            .get_result::<Task>(&mut conn)
            .expect("Failed to create parent task");

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
            .get_result::<Task>(&mut conn)
            .expect("Failed to create child task");

        // Create hierarchy
        diesel::insert_into(crate::schema::task_hierarchy::table)
            .values(NewTaskHierarchy {
                parent_task_id: Some(parent_task.id),
                child_task_id: Some(child_task.id),
            })
            .execute(&mut conn)
            .expect("Failed to create task hierarchy");

        // Verify hierarchy exists
        let hierarchy_exists = crate::schema::task_hierarchy::dsl::task_hierarchy
            .filter(crate::schema::task_hierarchy::dsl::child_task_id.eq(Some(child_task.id)))
            .first::<TaskHierarchy>(&mut conn)
            .optional()
            .expect("Failed to query task hierarchy")
            .is_some();

        assert!(hierarchy_exists, "Hierarchy should exist before detachment");

        // Call the detach_child_task function directly and await it
        let response = detach_child_task(State(state.clone()), axum::extract::Path(child_task.id))
            .await
            .expect("detach_child_task failed");

        assert_eq!(response, StatusCode::NO_CONTENT);

        // Verify hierarchy no longer exists
        let hierarchy_exists = crate::schema::task_hierarchy::dsl::task_hierarchy
            .filter(crate::schema::task_hierarchy::dsl::child_task_id.eq(Some(child_task.id)))
            .first::<TaskHierarchy>(&mut conn)
            .optional()
            .expect("Failed to query task hierarchy")
            .is_some();

        assert!(
            !hierarchy_exists,
            "Hierarchy should not exist after detachment"
        );
    }
}
