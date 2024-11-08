use diesel::prelude::*;
use diesel::result::Error as DieselError;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Deserialize)]
pub struct AttachChildRequest {
    pub parent_id: Option<i32>,
    pub child_id: i32,
}

pub trait HierarchyItem
where
    Self: Sized,
{
    type Id: Copy + Eq + PartialEq + Clone;

    fn get_parent_id(&self) -> Option<Self::Id>;
    fn get_child_id(&self) -> Self::Id;

    fn set_parent_id(&mut self, parent_id: Option<Self::Id>);
    fn set_child_id(&mut self, child_id: Self::Id);

    fn find_by_child_id(conn: &mut PgConnection, child_id: Self::Id) -> QueryResult<Option<Self>>;

    fn insert_new(conn: &mut PgConnection, item: &Self) -> QueryResult<()>;

    fn update_existing(conn: &mut PgConnection, item: &Self) -> QueryResult<()>;
}

#[derive(Debug)]
pub struct BasicTreeNode<T> {
    pub id: i32,
    pub data: T,
    pub children: Vec<BasicTreeNode<T>>,
}

// Generic function that builds a basic tree structure
pub fn build_generic_tree<T>(
    items: &[(i32, T)],
    hierarchies: &[(i32, i32)], // (child_id, parent_id)
) -> Vec<BasicTreeNode<T>>
where
    T: Clone,
{
    // Create a map of parent_id to children
    let mut parent_to_children: HashMap<i32, Vec<i32>> = HashMap::with_capacity(hierarchies.len());

    // Track which items are children
    let mut child_items: HashSet<i32> = HashSet::with_capacity(hierarchies.len());

    // Build the parent-to-children mapping
    for &(child_id, parent_id) in hierarchies {
        parent_to_children
            .entry(parent_id)
            .or_default()
            .push(child_id);
        child_items.insert(child_id);
    }

    // Create a map of item id to data for easy lookup
    let items_map: HashMap<i32, &T> = items.iter().map(|(id, data)| (*id, data)).collect();

    // Function to recursively build the tree
    fn build_subtree<T: Clone>(
        item_id: i32,
        items_map: &HashMap<i32, &T>,
        parent_to_children: &HashMap<i32, Vec<i32>>,
    ) -> BasicTreeNode<T> {
        let children = if let Some(child_ids) = parent_to_children.get(&item_id) {
            let mut children_vec = Vec::with_capacity(child_ids.len());
            for &child_id in child_ids {
                children_vec.push(build_subtree(child_id, items_map, parent_to_children));
            }
            children_vec
        } else {
            Vec::new()
        };

        BasicTreeNode {
            id: item_id,
            data: items_map[&item_id].clone(),
            children,
        }
    }

    // Build trees starting from root items (items that aren't children)
    let mut tree: Vec<BasicTreeNode<T>> = items
        .iter()
        .filter(|(id, _)| !child_items.contains(id))
        .map(|(id, _)| build_subtree(*id, &items_map, &parent_to_children))
        .collect();

    // Sort the tree by item ID for consistent ordering
    tree.sort_by_key(|node| node.id);

    tree
}

// Generic function to detect circular references
pub fn is_circular_reference<F, T>(start_id: T, mut get_parent_fn: F) -> Result<bool, DieselError>
where
    F: FnMut(T) -> Result<Option<T>, DieselError>,
    T: PartialEq + Clone,
{
    let mut current_parent_id = Some(start_id.clone());
    while let Some(pid) = current_parent_id {
        current_parent_id = get_parent_fn(pid)?;
        if current_parent_id == Some(start_id.clone()) {
            return Ok(true); // Circular reference detected
        }
    }
    Ok(false)
}

pub fn is_circular_hierarchy<F, T>(
    conn: &mut PgConnection,
    child_id: T,
    potential_parent_id: Option<T>,
    mut get_parent_fn: F,
) -> Result<bool, DieselError>
where
    F: FnMut(&mut PgConnection, T) -> Result<Option<T>, DieselError>,
    T: PartialEq + Clone,
{
    if let Some(p_id) = potential_parent_id {
        if p_id == child_id {
            // Immediate cycle detected
            return Ok(true);
        }
        // Traverse up the hierarchy to check for cycles
        let mut current_parent_id = Some(p_id);
        while let Some(current_id) = current_parent_id {
            if current_id == child_id {
                // Cycle detected
                return Ok(true);
            }
            current_parent_id = get_parent_fn(conn, current_id)?;
        }
    }
    Ok(false)
}

pub fn attach_child<H>(
    is_circular_fn: impl Fn(&mut PgConnection, H::Id, Option<H::Id>) -> Result<bool, DieselError>,
    item: H,
    conn: &mut PgConnection,
) -> Result<(), DieselError>
where
    H: HierarchyItem,
{
    let child_id = item.get_child_id();
    let parent_id = item.get_parent_id();

    // Prevent circular hierarchy
    if let Some(pid) = parent_id {
        if is_circular_fn(conn, child_id, Some(pid))? {
            return Err(DieselError::RollbackTransaction);
        }
    }

    // Check if hierarchy entry exists
    if let Some(mut existing_item) = H::find_by_child_id(conn, child_id)? {
        // Update existing hierarchy entry
        existing_item.set_parent_id(parent_id);
        H::update_existing(conn, &existing_item)?;
    } else {
        // Insert new hierarchy entry
        H::insert_new(conn, &item)?;
    }

    Ok(())
}

// A generic function to delete an item from a hierarchical structure based on the identifier
pub fn detach_child<F>(
    delete_fn: F,
    child_id: i32,
    conn: &mut PgConnection,
) -> Result<(), DieselError>
where
    F: Fn(&mut PgConnection, i32) -> Result<usize, DieselError>,
{
    let num_deleted = delete_fn(conn, child_id)?;
    if num_deleted == 0 {
        return Err(DieselError::NotFound);
    }
    Ok(())
}
