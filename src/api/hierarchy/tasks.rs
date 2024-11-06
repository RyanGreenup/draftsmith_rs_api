use super::generics::{attach_child, build_generic_tree, is_circular_hierarchy, HierarchyItem, BasicTreeNode};
use crate::api::state::AppState;
use crate::api::AttachChildRequest;
use crate::tables::{NewTaskHierarchy, TaskHierarchy};
use axum::{extract::{Json, Path}, http::StatusCode, extract::State};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

impl HierarchyItem for TaskHierarchy {
    type Id = i32;

    fn get_parent_id(&self) -> Option<i32> {
        self.parent_task_id
    }

    fn get_child_id(&self) -> i32 {
        self.child_task_id.expect("child_task_id should not be None")
    }

    fn set_parent_id(&mut self, parent_id: Option<i32>) {
        self.parent_task_id = parent_id;
    }

    fn set_child_id(&mut self, child_id: i32) {
        self.child_task_id = Some(child_id);
    }

    fn find_by_child_id(conn: &mut PgConnection, child_id: i32) -> QueryResult<Option<Self>> {
        use crate::schema::task_hierarchy::dsl::*;

        task_hierarchy
            .filter(child_task_id.eq(child_id))
            .first::<TaskHierarchy>(conn)
            .optional()
    }

    fn insert_new(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::task_hierarchy;

        let new_item = NewTaskHierarchy {
            parent_task_id: item.parent_task_id,
            child_task_id: item.child_task_id,
        };

        diesel::insert_into(task_hierarchy::table)
            .values(&new_item)
            .execute(conn)
            .map(|_| ())
    }

    fn update_existing(conn: &mut PgConnection, item: &Self) -> QueryResult<()> {
        use crate::schema::task_hierarchy::dsl::*;

        diesel::update(task_hierarchy.filter(child_task_id.eq(item.get_child_id())))
            .set(parent_task_id.eq(item.get_parent_id()))
            .execute(conn)
            .map(|_| ())
    }

    #[tokio::test]
    async fn test_detach_child_task() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get database connection");

        conn.test_transaction::<_, DieselError, _>(|conn| {
            // Create parent note and task
            let parent_note = diesel::insert_into(crate::schema::notes::table)
                .values(NewNote {
                    title: "Parent Note",
                    content: "Parent content",
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                })
                .get_result::<Note>(conn)
                .expect("Error creating parent note");

            let parent_task = diesel::insert_into(crate::schema::tasks::table)
                .values(NewTask {
                    note_id: Some(parent_note.id),
                    status: "todo",
                    effort_estimate: Some(BigDecimal::from(1)),
                    actual_effort: None,
                    deadline: None,
                    priority: Some(1),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                    all_day: Some(false),
                    goal_relationship: None,
                })
                .get_result::<Task>(conn)
                .expect("Error creating parent task");

            // Create child note and task
            let child_note = diesel::insert_into(crate::schema::notes::table)
                .values(NewNote {
                    title: "Child Note",
                    content: "Child content",
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                })
                .get_result::<Note>(conn)
                .expect("Error creating child note");

            let child_task = diesel::insert_into(crate::schema::tasks::table)
                .values(NewTask {
                    note_id: Some(child_note.id),
                    status: "todo",
                    effort_estimate: Some(BigDecimal::from(1)),
                    actual_effort: None,
                    deadline: None,
                    priority: Some(2),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                    all_day: Some(false),
                    goal_relationship: None,
                })
                .get_result::<Task>(conn)
                .expect("Error creating child task");

            // Create initial hierarchy
            let hierarchy = diesel::insert_into(crate::schema::task_hierarchy::table)
                .values(NewTaskHierarchy {
                    parent_task_id: Some(parent_task.id),
                    child_task_id: Some(child_task.id),
                })
                .get_result::<TaskHierarchy>(conn)
                .expect("Error creating task hierarchy");

            // Verify hierarchy exists
            assert_eq!(hierarchy.parent_task_id, Some(parent_task.id));
            assert_eq!(hierarchy.child_task_id, Some(child_task.id));

            // Test detaching child
            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(detach_child_task(
                    State(state.clone()),
                    Path(child_task.id),
                ))
                .expect("Failed to detach child task");

            // Verify hierarchy was removed
            use crate::schema::task_hierarchy::dsl::*;
            let remaining_hierarchy = task_hierarchy
                .filter(child_task_id.eq(child_task.id))
                .first::<TaskHierarchy>(conn);

            assert!(matches!(remaining_hierarchy, Err(DieselError::NotFound)));

            Ok(())
        });
    }
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
        parent_task_id: payload.parent_note_id,
        child_task_id: Some(payload.child_note_id),
    };

    // Call the generic attach_child function with the specific implementation
    attach_child(is_circular_fn, item, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod task_hierarchy_tests {
    use super::*;
    use crate::api::tests::setup_test_state;
    use crate::tables::{NewNote, NewTask, Note, Task};
    use axum::extract::State;
    use axum::Json;
    use bigdecimal::BigDecimal;
    use diesel::result::Error as DieselError;

    #[tokio::test]
    async fn test_attach_child_task() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get database connection");

        conn.test_transaction::<_, DieselError, _>(|conn| {
            // Create parent note and task
            let parent_note = diesel::insert_into(crate::schema::notes::table)
                .values(NewNote {
                    title: "Parent Note",
                    content: "Parent content",
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                })
                .get_result::<Note>(conn)
                .expect("Error creating parent note");

            let parent_task = diesel::insert_into(crate::schema::tasks::table)
                .values(NewTask {
                    note_id: Some(parent_note.id),
                    status: "todo",
                    effort_estimate: Some(BigDecimal::from(1)),
                    actual_effort: None,
                    deadline: None,
                    priority: Some(1),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                    all_day: Some(false),
                    goal_relationship: None,
                })
                .get_result::<Task>(conn)
                .expect("Error creating parent task");

            // Create child note and task
            let child_note = diesel::insert_into(crate::schema::notes::table)
                .values(NewNote {
                    title: "Child Note",
                    content: "Child content",
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                })
                .get_result::<Note>(conn)
                .expect("Error creating child note");

            let child_task = diesel::insert_into(crate::schema::tasks::table)
                .values(NewTask {
                    note_id: Some(child_note.id),
                    status: "todo",
                    effort_estimate: Some(BigDecimal::from(1)),
                    actual_effort: None,
                    deadline: None,
                    priority: Some(2),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                    all_day: Some(false),
                    goal_relationship: None,
                })
                .get_result::<Task>(conn)
                .expect("Error creating child task");

            // Test attaching child to parent
            let request = AttachChildRequest {
                parent_note_id: Some(parent_task.id),
                child_note_id: child_task.id,
            };

            tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(attach_child_task(
                    State(state.clone()),
                    Json(request),
                ))
                .expect("Failed to attach child task");

            // Verify the hierarchy was created
            use crate::schema::task_hierarchy::dsl::*;
            let hierarchy = task_hierarchy
                .filter(child_task_id.eq(child_task.id))
                .first::<TaskHierarchy>(conn)
                .expect("Failed to find task hierarchy");

            assert_eq!(hierarchy.parent_task_id, Some(parent_task.id));
            assert_eq!(hierarchy.child_task_id, Some(child_task.id));

            Ok(())
        });
    }
}

#[debug_handler]
pub async fn get_task_tree(
    State(state): State<AppState>,
) -> Result<Json<Vec<TaskTreeNode>>, StatusCode> {
    use crate::schema::task_hierarchy::dsl::task_hierarchy;
    use crate::tables::Task;
    use crate::schema::tasks::dsl::tasks;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get all tasks
    let all_tasks = tasks
        .load::<Task>(&mut conn)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    // Build the basic tree
    let basic_tree = build_generic_tree(&task_data, &hierarchy_tuples);

    // Convert BasicTreeNode to TaskTreeNode
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

    let tree = basic_tree.into_iter().map(convert_to_task_tree).collect();

    Ok(Json(tree))
}

pub async fn detach_child_task(
    State(state): State<AppState>,
    Path(child_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    use crate::schema::task_hierarchy::dsl::*;

    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Define specific delete logic for the task hierarchy
    let delete_fn = |conn: &mut PgConnection, child_id: i32| {
        diesel::delete(task_hierarchy.filter(child_task_id.eq(child_id))).execute(conn)
    };

    detach_child(delete_fn, child_id, &mut conn).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::tests::setup_test_state;
    use crate::tables::{NewNote, NewTask, Note, Task};
    use bigdecimal::BigDecimal;
    use diesel::result::Error as DieselError;

    #[tokio::test]
    async fn test_get_task_tree() {
        let state = setup_test_state();
        let mut conn = state
            .pool
            .get()
            .expect("Failed to get database connection");

        conn.test_transaction::<_, DieselError, _>(|conn| {
            // Create parent note and task
            let parent_note = diesel::insert_into(crate::schema::notes::table)
                .values(NewNote {
                    title: "Parent Note",
                    content: "Parent content",
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                })
                .get_result::<Note>(conn)
                .expect("Error creating parent note");

            let parent_task = diesel::insert_into(crate::schema::tasks::table)
                .values(NewTask {
                    note_id: Some(parent_note.id),
                    status: "todo",
                    effort_estimate: Some(BigDecimal::from(1)),
                    actual_effort: None,
                    deadline: None,
                    priority: Some(1),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                    all_day: Some(false),
                    goal_relationship: None,
                })
                .get_result::<Task>(conn)
                .expect("Error creating parent task");

            // Create child note and task
            let child_note = diesel::insert_into(crate::schema::notes::table)
                .values(NewNote {
                    title: "Child Note",
                    content: "Child content",
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                })
                .get_result::<Note>(conn)
                .expect("Error creating child note");

            let child_task = diesel::insert_into(crate::schema::tasks::table)
                .values(NewTask {
                    note_id: Some(child_note.id),
                    status: "todo",
                    effort_estimate: Some(BigDecimal::from(1)),
                    actual_effort: None,
                    deadline: None,
                    priority: Some(2),
                    created_at: Some(chrono::Utc::now().naive_utc()),
                    modified_at: Some(chrono::Utc::now().naive_utc()),
                    all_day: Some(false),
                    goal_relationship: None,
                })
                .get_result::<Task>(conn)
                .expect("Error creating child task");

            // Create task hierarchy
            let hierarchy = diesel::insert_into(crate::schema::task_hierarchy::table)
                .values(NewTaskHierarchy {
                    parent_task_id: Some(parent_task.id),
                    child_task_id: Some(child_task.id),
                })
                .get_result::<TaskHierarchy>(conn)
                .expect("Error creating task hierarchy");

            // Test getting task tree
            let task_tree = tokio::runtime::Runtime::new()
                .unwrap()
                .block_on(get_task_tree(State(state.clone())))
                .expect("Failed to get task tree");

            // Verify tree structure
            let root_nodes = task_tree.0;
            assert_eq!(root_nodes.len(), 1, "Expected one root node");

            let root_node = &root_nodes[0];
            assert_eq!(root_node.id, parent_task.id);
            assert_eq!(root_node.note_id, Some(parent_note.id));
            assert_eq!(root_node.status, "todo");
            assert_eq!(root_node.priority, Some(1));

            assert_eq!(root_node.children.len(), 1, "Expected one child node");
            let child_node = &root_node.children[0];
            assert_eq!(child_node.id, child_task.id);
            assert_eq!(child_node.note_id, Some(child_note.id));
            assert_eq!(child_node.status, "todo");
            assert_eq!(child_node.priority, Some(2));
            assert_eq!(child_node.children.len(), 0, "Child should have no children");

            Ok(())
        });
    }
}