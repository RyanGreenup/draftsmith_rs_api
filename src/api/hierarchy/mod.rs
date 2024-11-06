use serde::{Deserialize, Serialize};
pub mod generics;
pub mod notes;
pub mod tags;
pub mod tasks;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TagTreeNode {
    pub id: i32,
    pub name: String,
    pub children: Vec<TagTreeNode>,
}

// Modify Tag Hierarchy

// Modify Task Hierarchy
