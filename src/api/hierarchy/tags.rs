use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TagTreeNode {
    pub id: i32,
    pub name: String,
    pub children: Vec<TagTreeNode>,
}
