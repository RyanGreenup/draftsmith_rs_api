use super::generics::{attach_child, build_generic_tree, is_circular_hierarchy, HierarchyItem};
use crate::api::state::AppState;
use crate::api::AttachChildRequest;
use crate::tables::{NewTaskHierarchy, TaskHierarchy};
use axum::{extract::Json, http::StatusCode, extract::State};
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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskTreeNode {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub children: Vec<TaskTreeNode>,
}
