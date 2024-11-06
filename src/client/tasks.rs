use crate::tables::Task;
use crate::TASK_API;
use reqwest::{self, StatusCode};
use serde::{Deserialize, Serialize};
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

#[derive(Debug, Serialize)]
pub struct CreateTaskRequest {
    pub note_id: Option<i32>,
    pub status: String,
    pub effort_estimate: Option<String>,
    pub actual_effort: Option<String>,
    pub deadline: Option<chrono::NaiveDateTime>,
    pub priority: Option<i32>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct UpdateTaskRequest {
    pub note_id: Option<i32>,
    pub status: Option<String>,
    pub effort_estimate: Option<String>,
    pub actual_effort: Option<String>,
    pub deadline: Option<chrono::NaiveDateTime>,
    pub priority: Option<i32>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct HierarchyMapping {
    pub parent_id: Option<i32>,
    pub child_id: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TaskTreeNode {
    pub id: i32,
    pub note_id: Option<i32>,
    pub status: String,
    pub effort_estimate: Option<i32>,
    pub actual_effort: Option<i32>,
    pub deadline: Option<chrono::NaiveDateTime>,
    pub priority: Option<i32>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<String>,
    pub children: Vec<TaskTreeNode>,
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

#[derive(Debug, Serialize)]
pub struct AttachChildRequest {
    pub parent_id: Option<i32>,
    pub child_id: i32,
}

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
            effort_estimate: Some("2".to_string()),
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

        Ok(())
    }

    #[tokio::test]
    async fn test_update_task() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a task to update
        let task = CreateTaskRequest {
            note_id: None,
            status: "todo".to_string(),
            effort_estimate: Some("2".to_string()),
            actual_effort: None,
            deadline: None,
            priority: Some(1),
            all_day: Some(false),
            goal_relationship: None,
        };

        let created_task = create_task(base_url, task).await?;

        // Update the task
        let update = UpdateTaskRequest {
            note_id: None,
            status: Some("in_progress".to_string()),
            effort_estimate: Some("3".to_string()),
            actual_effort: Some("1".to_string()),
            deadline: None,
            priority: Some(2),
            all_day: Some(false),
            goal_relationship: None,
        };

        let updated_task = update_task(base_url, created_task.id, update).await?;
        assert_eq!(updated_task.status, "in_progress");
        assert_eq!(
            updated_task.effort_estimate,
            Some(bigdecimal::BigDecimal::from(3))
        );
        assert_eq!(updated_task.priority, Some(2));

        Ok(())
    }

    #[tokio::test]
    async fn test_delete_task() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a task to delete
        let task = CreateTaskRequest {
            note_id: None,
            status: "todo".to_string(),
            effort_estimate: Some("2".to_string()),
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

        // Clean up existing tasks and hierarchies first
        let tasks = fetch_tasks(base_url).await?;
        for task in tasks {
            delete_task(base_url, task.id).await?;
        }

        // Test fetching hierarchy mappings (should be empty initially)
        let initial_mappings = fetch_hierarchy_mappings(base_url).await?;
        assert!(initial_mappings.is_empty());

        // Create parent task
        let parent_task = create_task(
            base_url,
            CreateTaskRequest {
                note_id: None,
                status: "todo".to_string(),
                effort_estimate: Some("2".to_string()),
                actual_effort: None,
                deadline: None,
                priority: Some(1),
                all_day: Some(false),
                goal_relationship: None,
            },
        )
        .await?;

        // Create child task
        let child_task = create_task(
            base_url,
            CreateTaskRequest {
                note_id: None,
                status: "todo".to_string(),
                effort_estimate: Some("1".to_string()),
                actual_effort: None,
                deadline: None,
                priority: Some(2),
                all_day: Some(false),
                goal_relationship: None,
            },
        )
        .await?;

        // Attach child to parent
        let attach_request = AttachChildRequest {
            parent_id: Some(parent_task.id),
            child_id: child_task.id,
        };
        attach_child_task(base_url, attach_request).await?;

        // Verify tree structure
        let tree = fetch_task_tree(base_url).await?;
        let parent_node = tree.iter().find(|n| n.id == parent_task.id).unwrap();
        assert_eq!(parent_node.children.len(), 1);
        assert_eq!(parent_node.children[0].id, child_task.id);

        // Detach child
        detach_child_task(base_url, child_task.id).await?;

        // Verify detachment
        let updated_tree = fetch_task_tree(base_url).await?;
        let parent_node = updated_tree
            .iter()
            .find(|n| n.id == parent_task.id)
            .unwrap();
        assert_eq!(parent_node.children.len(), 0);

        Ok(())
    }
}
