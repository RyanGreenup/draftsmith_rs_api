use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::collections::{HashMap, HashSet};

pub trait HierarchyItem {
    type Id: Copy + Eq;

    fn get_parent_id(&self) -> Option<Self::Id>;
    fn get_child_id(&self) -> Self::Id;

    fn set_parent_id(&mut self, parent_id: Option<Self::Id>);
    fn set_child_id(&mut self, child_id: Self::Id);

    fn find_by_child_id(conn: &mut PgConnection, child_id: Self::Id) -> QueryResult<Option<Self>>
    where
        Self: Sized;

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
    let mut parent_to_children: HashMap<Option<i32>, Vec<i32>> = HashMap::new();

    // Track which items are children
    let mut child_items: HashSet<i32> = HashSet::new();

    // Build the parent-to-children mapping
    for (child_id, parent_id) in hierarchies {
        parent_to_children
            .entry(Some(*parent_id))
            .or_default()
            .push(*child_id);
        child_items.insert(*child_id);
    }

    // Create a map of item id to data for easy lookup
    let items_map: HashMap<_, _> = items
        .iter()
        .map(|(item_id, data)| (*item_id, data))
        .collect();

    // Function to recursively build the tree
    fn build_subtree<T: Clone>(
        item_id: i32,
        items_map: &HashMap<i32, &T>,
        parent_to_children: &HashMap<Option<i32>, Vec<i32>>,
    ) -> BasicTreeNode<T> {
        let children = parent_to_children
            .get(&Some(item_id))
            .map(|children| {
                children
                    .iter()
                    .map(|child_id| build_subtree(*child_id, items_map, parent_to_children))
                    .collect()
            })
            .unwrap_or_default();

        BasicTreeNode {
            id: item_id,
            data: (*items_map.get(&item_id).unwrap()).clone(),
            children,
        }
    }

    // Build trees starting from root items (items that aren't children)
    let mut tree: Vec<BasicTreeNode<T>> = items
        .iter()
        .filter(|(item_id, _)| !child_items.contains(item_id))
        .map(|(item_id, _)| build_subtree(*item_id, &items_map, &parent_to_children))
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

// Concrete implementation specific to your use case
pub fn is_circular_hierarchy(
    conn: &mut PgConnection,
    _child_id: i32,
    potential_parent_id: Option<i32>,
) -> Result<bool, DieselError> {
    use crate::schema::note_hierarchy::dsl::*;

    if let Some(potential_pid) = potential_parent_id {
        is_circular_reference(potential_pid, |pid| {
            note_hierarchy
                .filter(child_note_id.eq(pid))
                .select(parent_note_id)
                .first::<Option<i32>>(conn)
                .optional()
                .map(|opt| opt.flatten())
        })
    } else {
        Ok(false)
    }
}

pub fn attach_child<F, A>(
    is_circular_fn: F,
    attach_fn: A,
    child_id: i32,
    parent_id: Option<i32>,
    conn: &mut PgConnection,
) -> Result<(), DieselError>
where
    F: Fn(&mut PgConnection, i32, Option<i32>) -> Result<bool, DieselError>,
    A: Fn(&mut PgConnection, i32, Option<i32>) -> Result<(), DieselError>,
{
    // Prevent circular hierarchy
    if let Some(parent_id) = parent_id {
        if is_circular_fn(conn, child_id, Some(parent_id))? {
            return Err(DieselError::NotFound); // Handle appropriately
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
