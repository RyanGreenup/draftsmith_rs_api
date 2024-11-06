use crate::tables::Task;
use crate::TASK_API;
pub use crate::api::hierarchy::tasks::{AttachChildRequest, TaskTreeNode};
pub use crate::api::tasks::{CreateTaskRequest, UpdateTaskRequest};
use bigdecimal::BigDecimal;
use reqwest::{self, StatusCode};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Task not found")]
    NotFound(i32),

    #[error("Unexpected server error: {0}")]
    ServerError(String),
}

// * Types ....................................................................



#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HierarchyMapping {
    pub parent_id: Option<i32>,
    pub child_id: i32,
}

// * Client ...................................................................
// ** Flat Functions ..........................................................

pub async fn create_task(base_url: &str, task: CreateTaskRequest) -> Result<Task, TaskError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{TASK_API}", base_url);
    let response = client.post(url).json(&task).send().await?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TaskError::NotFound(-1));
    }

    let created_task = response.error_for_status()?.json::<Task>().await?;
    Ok(created_task)
}

pub async fn fetch_task(base_url: &str, id: i32) -> Result<Task, TaskError> {
    let url = format!("{}/{TASK_API}/{}", base_url, id);
    let response = reqwest::get(url).await?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TaskError::NotFound(id));
    }

    let task = response.error_for_status()?.json::<Task>().await?;
    Ok(task)
}

pub async fn fetch_tasks(base_url: &str) -> Result<Vec<Task>, TaskError> {
    let url = format!("{}/{TASK_API}", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let tasks = response.json::<Vec<Task>>().await?;
    Ok(tasks)
}

pub async fn update_task(
    base_url: &str,
    id: i32,
    task: UpdateTaskRequest,
) -> Result<Task, TaskError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{TASK_API}/{}", base_url, id);
    let response = client.put(url).json(&task).send().await?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TaskError::NotFound(id));
    }

    let updated_task = response.error_for_status()?.json::<Task>().await?;
    Ok(updated_task)
}

pub async fn delete_task(base_url: &str, id: i32) -> Result<(), TaskError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{TASK_API}/{}", base_url, id);
    let response = client.delete(url).send().await?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TaskError::NotFound(id));
    }

    response.error_for_status()?;
    Ok(())
}

// ** Hierarchical Functions ..................................................

pub async fn attach_child_task(
    base_url: &str,
    payload: AttachChildRequest,
) -> Result<(), TaskError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tasks/hierarchy/attach", base_url);
    client
        .post(url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()
        .map_err(TaskError::from)?;
    Ok(())
}

pub async fn detach_child_task(base_url: &str, child_task_id: i32) -> Result<(), TaskError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tasks/hierarchy/detach/{}", base_url, child_task_id);
    client
        .delete(url)
        .send()
        .await?
        .error_for_status()
        .map_err(TaskError::from)?;
    Ok(())
}

pub async fn fetch_task_tree(base_url: &str) -> Result<Vec<TaskTreeNode>, TaskError> {
    let url = format!("{}/tasks/tree", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let task_tree = response.json::<Vec<TaskTreeNode>>().await?;
    Ok(task_tree)
}

pub async fn update_task_tree(base_url: &str, tree: TaskTreeNode) -> Result<(), TaskError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tasks/tree", base_url);
    client
        .put(url)
        .json(&tree)
        .send()
        .await?
        .error_for_status()
        .map_err(TaskError::from)?;
    Ok(())
}

pub async fn fetch_hierarchy_mappings(base_url: &str) -> Result<Vec<HierarchyMapping>, TaskError> {
    let url = format!("{}/tasks/hierarchy", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let mappings = response.json::<Vec<HierarchyMapping>>().await?;
    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BASE_URL;
    use tokio;

    #[tokio::test]
    async fn test_create_and_fetch_task() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a test task
        let task = CreateTaskRequest {
            note_id: None,
            status: "todo".to_string(),
            effort_estimate: Some(BigDecimal::from_str("2").unwrap()),
            actual_effort: None,
            deadline: None,
            priority: Some(1),
            all_day: Some(false),
            goal_relationship: None,
        };

        let created_task = create_task(base_url, task).await?;
        assert!(!created_task.status.is_empty());

        // Fetch the created task
        let fetched_task = fetch_task(base_url, created_task.id).await?;
        assert_eq!(fetched_task.id, created_task.id);
        assert_eq!(fetched_task.status, "todo");
        assert_eq!(fetched_task.priority, Some(1));

        // Clean up the tasks we created
        delete_task(base_url, created_task.id).await?;
        Ok(())
    }



    #[tokio::test]
    async fn test_delete_task() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a task to delete
        let task = CreateTaskRequest {
            note_id: None,
            status: "todo".to_string(),
            effort_estimate: Some(BigDecimal::from_str("2").unwrap()),
            actual_effort: None,
            deadline: None,
            priority: Some(1),
            all_day: Some(false),
            goal_relationship: None,
        };

        let created_task = create_task(base_url, task).await?;

        // Delete the task
        delete_task(base_url, created_task.id).await?;

        // Verify deletion
        let result = fetch_task(base_url, created_task.id).await;
        assert!(matches!(result, Err(TaskError::NotFound(_))));

        Ok(())
    }

    #[tokio::test]
    async fn test_task_tree_operations() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        dbg!("Running task tree operations test");
        let initial_tree = fetch_task_tree(base_url).await?;
        let initial_parents = initial_tree.len();

        dbg!("Creating parent task");
        // Create parent task
        let parent_task = CreateTaskRequest {
            note_id: None,
            status: "todo".to_string(),
            effort_estimate: Some(BigDecimal::from_str("2").unwrap()),
            actual_effort: None,
            deadline: None,
            priority: Some(1),
            all_day: Some(false),
            goal_relationship: None,
        };
        let created_parent = create_task(base_url, parent_task).await?;

        dbg!("Creating child task");
        // Create child task
        let child_task = CreateTaskRequest {
            note_id: None,
            status: "todo".to_string(),
            effort_estimate: Some(BigDecimal::from_str("1").unwrap()),
            actual_effort: None,
            deadline: None,
            priority: Some(2),
            all_day: Some(false),
            goal_relationship: None,
        };
        let created_child = create_task(base_url, child_task).await?;

        dbg!("Attaching child to parent");
        // Attach child to parent
        let attach_request = AttachChildRequest {
            parent_task_id: Some(created_parent.id),
            child_task_id: created_child.id,
        };
        attach_child_task(base_url, attach_request).await?;

        dbg!("Fetching task tree");
        // Verify tree structure
        let tree = fetch_task_tree(base_url).await?;
        // This might fail as other tests might have created tasks (async pain)
        // assert_eq!(tree.len(), initial_parents+1); // Should be one more parent as the child is not
                                                   // at root

        // Verify the parent is at the root loevel
        let mut parent_found = false;
        for n in tree.iter() {
            if n.id == created_parent.id {
                assert_eq!(n.children.len(), 1);
                assert_eq!(n.children[0].id, created_child.id);
                parent_found = true;
            }
        }
        assert!(parent_found);

        dbg!("Fetching hierarchy mappings");
        // Verify hierarchy mappings
        let mut parent_child_found = false;
        let mappings = fetch_hierarchy_mappings(base_url).await?;
        for m in mappings.iter() {
            if let Some(parent_id) = m.parent_id {
                if parent_id == created_parent.id && m.child_id == created_child.id {
                    parent_child_found = true;
                }
            }
        }
        assert!(parent_child_found);

        dbg!("Detaching child from parent");
        // Detach child
        detach_child_task(base_url, created_child.id).await?;

        dbg!("Fetching task tree after detach");
        // Verify detachment
        let tree_after_detach = fetch_task_tree(base_url).await?;
        let mut child_parent_found = false;
        for n in tree_after_detach.iter() {
            if n.id == created_child.id {
                assert_eq!(n.children.len(), 0);
                child_parent_found = true;
            }
        }
        assert!(child_parent_found);

        Ok(())
    }
}
