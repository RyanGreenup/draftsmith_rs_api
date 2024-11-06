use serde::{Deserialize, Serialize};
pub mod generics;
pub mod notes;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TagTreeNode {
    pub id: i32,
    pub name: String,
    pub children: Vec<TagTreeNode>,
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

// Modify Tag Hierarchy

// Modify Task Hierarchy
