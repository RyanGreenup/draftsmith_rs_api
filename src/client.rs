pub use crate::api::{
    AttachChildRequest, BatchUpdateRequest, BatchUpdateResponse, CreateNoteRequest, NoteHash,
    NoteTreeNode, UpdateNoteRequest, compute_note_hash,
};
pub use crate::tables::{HierarchyMapping, NoteWithoutFts, NoteWithParent};
use crate::FLAT_API;
use std::fs;
use reqwest::Error as ReqwestError;
use std::collections::HashMap;
use std::fmt;

fn extract_parent_mapping(nodes: &[SimpleNode]) -> HashMap<i32, Option<i32>> {
    let mut parent_map = HashMap::new();

    fn process_node(
        node: &SimpleNode,
        parent_id: Option<i32>,
        map: &mut HashMap<i32, Option<i32>>,
    ) {
        map.insert(node.id, parent_id);
        for child in &node.children {
            process_node(child, Some(node.id), map);
        }
    }

    for node in nodes {
        process_node(node, None, &mut parent_map);
    }

    parent_map
}

#[derive(Debug)]
pub enum NoteError {
    NotFound(i32),
    RequestError(ReqwestError),
    IOError(std::io::Error),
    SerdeYamlError(serde_yaml::Error),
    SerdeJsonError(serde_json::Error),
}

impl fmt::Display for NoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteError::NotFound(id) => write!(f, "Note with id {} not found", id),
            NoteError::RequestError(e) => write!(f, "Request error: {}", e),
            NoteError::IOError(e) => write!(f, "IO error: {}", e),
            NoteError::SerdeYamlError(e) => write!(f, "YAML serialization error: {}", e),
            NoteError::SerdeJsonError(e) => write!(f, "JSON serialization error: {}", e),
        }
    }
}

impl std::error::Error for NoteError {}

impl From<ReqwestError> for NoteError {
    fn from(err: ReqwestError) -> Self {
        NoteError::RequestError(err)
    }
}

impl From<std::io::Error> for NoteError {
    fn from(err: std::io::Error) -> Self {
        NoteError::IOError(err)
    }
}

impl From<serde_yaml::Error> for NoteError {
    fn from(err: serde_yaml::Error) -> Self {
        NoteError::SerdeYamlError(err)
    }
}

impl From<serde_json::Error> for NoteError {
    fn from(err: serde_json::Error) -> Self {
        NoteError::SerdeJsonError(err)
    }
}

pub async fn fetch_note(
    base_url: &str,
    id: i32,
    metadata_only: bool,
) -> Result<NoteWithoutFts, NoteError> {
    let url = if metadata_only {
        format!("{}/{FLAT_API}/{}?metadata_only=true", base_url, id)
    } else {
        format!("{}/{FLAT_API}/{}", base_url, id)
    };

    let response = reqwest::get(url).await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(id));
    }

    let response = response.error_for_status()?;
    let note = response.json::<NoteWithoutFts>().await?;

    // If metadata_only is true, ensure content field is empty
    if metadata_only {
        Ok(NoteWithoutFts {
            content: String::new(),
            ..note
        })
    } else {
        Ok(note)
    }
}

pub async fn fetch_notes(
    base_url: &str,
    metadata_only: bool,
) -> Result<Vec<NoteWithoutFts>, NoteError> {
    let url = if metadata_only {
        format!("{}/{FLAT_API}?metadata_only=true", base_url)
    } else {
        format!("{}/{FLAT_API}", base_url)
    };
    let response = reqwest::get(url).await?.error_for_status()?;
    let notes = response.json::<Vec<NoteWithoutFts>>().await?;

    // If metadata_only is true, ensure content field is empty
    if metadata_only {
        Ok(notes
            .into_iter()
            .map(|mut note| {
                note.content = String::new();
                note
            })
            .collect())
    } else {
        Ok(notes)
    }
}

pub async fn create_note(
    base_url: &str,
    note: CreateNoteRequest,
) -> Result<NoteWithoutFts, NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}", base_url);
    let response = client
        .post(url)
        .json(&note)
        .send()
        .await?
        .error_for_status()?;
    let created_note = response.json::<NoteWithoutFts>().await?;
    Ok(created_note)
}

pub async fn update_note(
    base_url: &str,
    id: i32,
    note: UpdateNoteRequest,
) -> Result<NoteWithoutFts, NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}/{}", base_url, id);
    let response = client.put(url).json(&note).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(id));
    }

    let response = response.error_for_status()?;
    let updated_note = response.json::<NoteWithoutFts>().await?;
    Ok(updated_note)
}

pub async fn delete_note(base_url: &str, id: i32) -> Result<(), NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}/{}", base_url, id);
    let response = client.delete(url).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(id));
    }

    response.error_for_status()?;
    Ok(())
}

pub async fn attach_child_note(
    base_url: &str,
    payload: AttachChildRequest,
) -> Result<(), NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/notes/hierarchy/attach", base_url);
    client
        .post(url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()
        .map_err(NoteError::from)?;
    Ok(())
}

pub async fn detach_child_note(base_url: &str, child_note_id: i32) -> Result<(), NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/notes/hierarchy/detach/{}", base_url, child_note_id);
    client
        .delete(url)
        .send()
        .await?
        .error_for_status()
        .map_err(NoteError::from)?;
    Ok(())
}

pub async fn fetch_hierarchy_mappings(base_url: &str) -> Result<Vec<HierarchyMapping>, NoteError> {
    let url = format!("{}/notes/hierarchy", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let mappings = response.json::<Vec<HierarchyMapping>>().await?;
    Ok(mappings)
}

pub async fn fetch_note_tree(base_url: &str) -> Result<Vec<NoteTreeNode>, NoteError> {
    let url = format!("{}/notes/tree", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let note_tree = response.json::<Vec<NoteTreeNode>>().await?;
    Ok(note_tree)
}

pub async fn update_note_tree(base_url: &str, tree: NoteTreeNode) -> Result<(), NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/notes/tree", base_url);
    client
        .put(url)
        .json(&tree)
        .send()
        .await?
        .error_for_status()
        .map_err(NoteError::from)?;
    Ok(())
}

pub async fn get_note_hash(base_url: &str, note_id: i32) -> Result<String, NoteError> {
    let url = format!("{}/notes/flat/{}/hash", base_url, note_id);
    let response = reqwest::get(url).await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(note_id));
    }

    let response = response.error_for_status()?;
    let hash = response.text().await?;
    Ok(hash)
}

pub async fn get_all_note_hashes(base_url: &str) -> Result<Vec<NoteHash>, NoteError> {
    let url = format!("{}/notes/flat/hashes", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let hashes = response.json::<Vec<NoteHash>>().await?;
    Ok(hashes)
}

pub async fn batch_update_notes(
    base_url: &str,
    updates: Vec<(i32, UpdateNoteRequest)>,
) -> Result<BatchUpdateResponse, NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}/batch", base_url);
    let payload = BatchUpdateRequest { updates };
    let response = client
        .put(url)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    let result = response.json::<BatchUpdateResponse>().await?;
    Ok(result)
}

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct SimpleNode {
    pub id: i32,
    #[allow(unused)]
    pub title: String,
    pub children: Vec<SimpleNode>,
}

pub fn write_hierarchy_to_yaml(
    tree: &[NoteTreeNode],
    path: &std::path::Path,
) -> std::io::Result<()> {
    fn simplify_tree(node: &NoteTreeNode) -> SimpleNode {
        SimpleNode {
            id: node.id,
            title: node.title.clone().expect("Node title should not be None"),
            children: node.children.iter().map(simplify_tree).collect(),
        }
    }

    let simple_tree: Vec<SimpleNode> = tree.iter().map(simplify_tree).collect();
    let yaml = serde_yaml::to_string(&simple_tree)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(path, yaml)
}

#[allow(dead_code)]
fn simple_node_to_note_tree_node(
    simple_node: &SimpleNode,
    content_map: &HashMap<i32, String>,
) -> NoteTreeNode {
    NoteTreeNode {
        id: simple_node.id,
        title: Some(simple_node.title.clone()),
        content: content_map.get(&simple_node.id).cloned(),
        created_at: None,
        modified_at: None,
        hierarchy_type: Some("block".to_string()),
        children: simple_node
            .children
            .iter()
            .map(|child| simple_node_to_note_tree_node(child, content_map))
            .collect(),
    }
}

async fn read_notes_to_vec(base_url: &str, dir_path: &std::path::Path) -> Result<Vec<(i32, UpdateNoteRequest)>, NoteError> {

    // Get server-side hashes
    let server_hashes = get_all_note_hashes(base_url).await?;
    let server_hash_map: HashMap<i32, String> =
        server_hashes.into_iter().map(|h| (h.id, h.hash)).collect();


    // Read all .md files in the directory
    let mut notes_to_update = Vec::new();
    for entry in fs::read_dir(dir_path).map_err(NoteError::IOError)? {
        let entry = entry.map_err(NoteError::IOError)?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "md") {
            // Extract note ID from filename
            let id = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|s| s.parse::<i32>().ok())
                .ok_or_else(|| {
                    NoteError::IOError(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "Failed to parse note ID from filename",
                    ))
                })?;

            // Read note content
            let content = fs::read_to_string(&path).map_err(NoteError::IOError)?;

            // Add to notes to update if the server hash differs
            if let Some(_server_hash) = server_hash_map.get(&id) {
                notes_to_update.push((
                    id,
                    UpdateNoteRequest {
                        title: None,
                        content,
                    },
                ));
            }
        }
    }

    Ok(notes_to_update)
}

pub async fn read_from_disk(base_url: &str, dir_path: &std::path::Path) -> Result<(), NoteError> {
    // Step 1. Update Content..................................................

    // Get the notes from disk regardless of hierarchy
    let notes_with_content_to_update = read_notes_to_vec(base_url, dir_path).await?;


    // Get local note hashes using the API function that considers hierarchy
    let client = reqwest::Client::new();
    // TODO pull from /notes/flat/hashes
    let url = format!("{}/notes/flat/hashes", base_url);
    // Output type defined in api.rs by this type signature:
        // From api.rs
        // async fn get_all_note_hashes(
        //     State(state): State<AppState>,
        // ) -> Result<Json<Vec<NoteHash>>, StatusCode> {

    // If the hash has changed, then the hierarchy has also changed (server accounts for this)

    // Now update notes on server that have changed using
    // 1. batch_update_notes

    // Step 2. Update Hierarchy................................................
    // Set the content and title as Option::None so it's not sent again

    // Read metadata.yaml to get hierarchy information
    let metadata_path = dir_path.join("metadata.yaml");
    let metadata_content = fs::read_to_string(metadata_path).map_err(NoteError::IOError)?;
    let tree: Vec<SimpleNode> =
        serde_yaml::from_str(&metadata_content).map_err(NoteError::SerdeYamlError)?;

    // Compare this to the current hierarchy on the server
    let hierarchy_mappings = fetch_hierarchy_mappings(base_url).await?;

    // TODO: attach and detach as needed (If we need to walk the tree, use a 
    // 2. attach_child_note
    // 3. detach_child_note

    Ok(())
}

pub async fn write_notes_to_disk(
    notes: &[NoteWithoutFts],
    tree: &[NoteTreeNode],
    output_dir: &std::path::Path,
) -> std::io::Result<()> {
    use futures::future::join_all;
    use tokio::fs;

    // Create a vector of futures for writing note files
    let write_futures: Vec<_> = notes
        .iter()
        .map(|note| {
            let file_path = output_dir.join(format!("{}.md", note.id));
            let content = note.content.clone();
            async move { fs::write(file_path, content).await }
        })
        .collect();

    // Write all note files concurrently
    join_all(write_futures)
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    fn simplify_tree(node: &NoteTreeNode) -> SimpleNode {
        SimpleNode {
            id: node.id,
            title: node.title.clone().expect("Node title should not be None"),
            children: node.children.iter().map(simplify_tree).collect(),
        }
    }

    let simple_tree: Vec<SimpleNode> = tree.iter().map(simplify_tree).collect();

    // Save the simplified hierarchy as metadata.yaml
    let metadata_path = output_dir.join("metadata.yaml");
    let yaml = serde_yaml::to_string(&simple_tree)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(metadata_path, yaml).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BASE_URL;

    #[tokio::test]
    async fn test_fetch_notes() {
        let base_url = BASE_URL;
        let result = fetch_notes(base_url, false).await;
        assert!(result.is_ok());
        let notes = result.unwrap();
        assert!(!notes.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_note() {
        let base_url = BASE_URL;
        // First get all notes to find a valid ID
        let notes = fetch_notes(base_url, false).await.unwrap();
        let first_note_id = notes[0].id;

        let result = fetch_note(base_url, first_note_id, false).await;
        assert!(result.is_ok());
        let note = result.unwrap();
        assert_eq!(note.id, first_note_id);
    }

    #[tokio::test]
    async fn test_create_note() {
        let base_url = BASE_URL;
        let note = CreateNoteRequest {
            title: "Test Note".to_string(),
            content: "This is a test note".to_string(),
        };

        let result = create_note(base_url, note).await;
        assert!(result.is_ok());
        let created_note = result.unwrap();
        assert!(!created_note.title.is_empty());
        assert!(!created_note.content.is_empty());
    }

    #[tokio::test]
    async fn test_update_note() {
        let base_url = BASE_URL;
        // First create a note to update
        let create_note_req = CreateNoteRequest {
            title: "Test Note".to_string(),
            content: "This is a test note".to_string(),
        };
        let created_note = create_note(base_url, create_note_req).await.unwrap();

        // Now update it
        let update_note_req = UpdateNoteRequest {
            title: Some("Updated Test Note".to_string()),
            content: "This is an updated test note".to_string(),
        };
        let result = update_note(base_url, created_note.id, update_note_req).await;
        assert!(result.is_ok());
        let updated_note = result.unwrap();
        assert_eq!(updated_note.id, created_note.id);
        // Title is now automatically set as H1 of content by Database
        // See commit 12acc9fb1b177b279181c4d15618e60571722ca1
        // assert_eq!(updated_note.title, "Updated Test Note");
        assert_eq!(updated_note.content, "This is an updated test note");
    }

    #[tokio::test]
    async fn test_fetch_notes_metadata_only() {
        let base_url = BASE_URL;
        let result = fetch_notes(base_url, true).await;
        if let Err(ref e) = result {
            eprintln!("Error fetching notes: {}", e);
            if let NoteError::RequestError(req_err) = e {
                if let Some(status) = req_err.status() {
                    eprintln!("Status code: {}", status);
                }
                if let Some(url) = req_err.url() {
                    eprintln!("URL: {}", url);
                }
            }
        }
        assert!(result.is_ok());
        let notes = result.unwrap();
        assert!(!notes.is_empty());
        // Verify content field is empty in metadata-only response
        assert!(notes[0].content.is_empty());
    }

    #[tokio::test]
    async fn test_delete_note() {
        let base_url = BASE_URL;
        // First create a note to delete
        let create_note_req = CreateNoteRequest {
            title: "Test Note".to_string(),
            content: "This is a test note".to_string(),
        };
        let created_note = create_note(base_url, create_note_req).await.unwrap();

        // Now delete it
        let result = delete_note(base_url, created_note.id).await;
        assert!(result.is_ok());

        // Verify the note was deleted by trying to fetch it
        let fetch_result = fetch_note(base_url, created_note.id, false).await;
        assert!(matches!(fetch_result, Err(NoteError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_attach_and_detach_child_note() {
        let base_url = BASE_URL;

        // Create parent note
        let parent_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Parent Note".to_string(),
                content: "This is the parent note".to_string(),
            },
        )
        .await
        .unwrap();

        // Create child note
        let child_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child Note".to_string(),
                content: "This is the child note".to_string(),
            },
        )
        .await
        .unwrap();

        // Attach child note to parent note with a valid hierarchy type
        let attach_request = AttachChildRequest {
            child_note_id: child_note.id,
            parent_note_id: Some(parent_note.id),
            hierarchy_type: Some("block".to_string()),
        };
        let attach_result = attach_child_note(base_url, attach_request).await;
        assert!(attach_result.is_ok(), "Failed to attach child note");

        // Give the server more time to process the attachment
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Fetch the note tree and verify the hierarchy
        let note_tree = fetch_note_tree(base_url)
            .await
            .expect("Failed to fetch note tree");

        // Function to find a node in the tree by ID
        fn find_node(tree: &[NoteTreeNode], id: i32) -> Option<&NoteTreeNode> {
            for node in tree {
                if node.id == id {
                    return Some(node);
                }
                if let Some(found) = find_node(&node.children, id) {
                    return Some(found);
                }
            }
            None
        }

        // Verify that the child note is now under the parent note
        let parent_node = find_node(&note_tree, parent_note.id).expect("Parent note not found");
        let child_found = find_node(&parent_node.children, child_note.id).is_some();

        if !child_found {
            // Print debug information
            println!("Parent node children: {:?}", parent_node.children);
            println!("Looking for child ID: {}", child_note.id);
            assert!(
                false,
                "Child note {} not found under parent {}",
                child_note.id, parent_note.id
            );
        }

        // Detach the child note
        let detach_result = detach_child_note(base_url, child_note.id).await;
        assert!(detach_result.is_ok(), "Failed to detach child note");

        // Give the server a moment to process the detachment
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Fetch the note tree again and verify the child note is detached
        let updated_note_tree = fetch_note_tree(base_url).await.unwrap();
        let parent_node =
            find_node(&updated_note_tree, parent_note.id).expect("Parent note not found");
        assert!(
            find_node(&parent_node.children, child_note.id).is_none(),
            "Child note still attached after detachment"
        );
    }

    #[tokio::test]
    async fn test_update_note_tree() {
        let base_url = BASE_URL;

        // Create root note
        let root_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Root Note".to_string(),
                content: "Root content".to_string(),
            },
        )
        .await
        .unwrap();

        // Create child notes
        let child1_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child 1".to_string(),
                content: "Child 1 content".to_string(),
            },
        )
        .await
        .unwrap();

        let child2_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child 2".to_string(),
                content: "Child 2 content".to_string(),
            },
        )
        .await
        .unwrap();

        // Create a tree structure
        let tree = NoteTreeNode {
            id: root_note.id,
            title: Some("Updated Root".to_string()),
            content: Some("Updated root content".to_string()),
            created_at: None,
            modified_at: None,
            hierarchy_type: None,
            children: vec![
                NoteTreeNode {
                    id: child1_note.id,
                    title: Some("Updated Child 1".to_string()),
                    content: Some("Updated child 1 content".to_string()),
                    created_at: None,
                    modified_at: None,
                    hierarchy_type: Some("block".to_string()),
                    children: vec![],
                },
                NoteTreeNode {
                    id: child2_note.id,
                    title: Some("Updated Child 2".to_string()),
                    content: Some("Updated child 2 content".to_string()),
                    created_at: None,
                    modified_at: None,
                    hierarchy_type: Some("block".to_string()),
                    children: vec![],
                },
            ],
        };

        // Update the tree structure
        let update_result = update_note_tree(base_url, tree.clone()).await;
        assert!(update_result.is_ok(), "Failed to update note tree");

        // Fetch the updated tree
        let fetched_tree = fetch_note_tree(base_url).await.unwrap();

        // Find our test tree in the fetched trees
        let updated_tree = fetched_tree
            .iter()
            .find(|n| n.id == root_note.id)
            .expect("Could not find updated tree");

        // Verify the structure
        // Titles are now automatically set as H1 of content by Database
        // [[file:migrations/2024-10-31-024911_create_notes/up.sql::CREATE OR REPLACE FUNCTION update_title_from_content()][Postgres set title as h1 content]]
        // assert_eq!(updated_tree.title, "Updated Root");
        // see commit 12acc9fb1b177b279181c4d15618e60571722ca1
        assert_eq!(
            updated_tree.content,
            Some("Updated root content".to_string())
        );
        assert_eq!(updated_tree.children.len(), 2);

        // Verify children
        let child1 = updated_tree
            .children
            .iter()
            .find(|n| n.id == child1_note.id)
            .expect("Could not find child1");
        // assert_eq!(child1.title, "Updated Child 1");
        assert_eq!(child1.content, Some("Updated child 1 content".to_string()));
        assert_eq!(child1.hierarchy_type, Some("block".to_string()));

        let child2 = updated_tree
            .children
            .iter()
            .find(|n| n.id == child2_note.id)
            .expect("Could not find child2");
        // assert_eq!(child2.title, "Updated Child 2");
        assert_eq!(child2.content, Some("Updated child 2 content".to_string()));
        assert_eq!(child2.hierarchy_type, Some("block".to_string()));
    }

    #[tokio::test]
    async fn test_attach_child_note_invalid_hierarchy_type() {
        let base_url = BASE_URL;

        // Create parent and child notes
        let parent_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Parent Note".to_string(),
                content: "This is the parent note".to_string(),
            },
        )
        .await
        .unwrap();

        let child_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child Note".to_string(),
                content: "This is the child note".to_string(),
            },
        )
        .await
        .unwrap();

        // Attempt to attach with an invalid hierarchy type
        let attach_request = AttachChildRequest {
            child_note_id: child_note.id,
            parent_note_id: Some(parent_note.id),
            hierarchy_type: Some("invalid_type".to_string()),
        };
        let attach_result = attach_child_note(base_url, attach_request).await;

        // Expecting an error due to invalid hierarchy type
        assert!(
            attach_result.is_err(),
            "Attachment should fail with invalid hierarchy type"
        );
    }

    #[tokio::test]
    async fn test_save_notes_to_directory() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a temporary directory for our test
        let temp_dir = tempfile::tempdir()?;

        // Create some test notes first
        let note1 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Note 1".to_string(),
                content: "Content 1".to_string(),
            },
        )
        .await?;

        let note2 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Note 2".to_string(),
                content: "Content 2".to_string(),
            },
        )
        .await?;

        // Attach note2 as child of note1
        let attach_request = AttachChildRequest {
            child_note_id: note2.id,
            parent_note_id: Some(note1.id),
            hierarchy_type: Some("block".to_string()),
        };
        attach_child_note(base_url, attach_request).await?;

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Fetch notes and tree
        let notes = fetch_notes(base_url, false).await?;
        let tree = fetch_note_tree(base_url).await?;

        // Write everything to disk
        write_notes_to_disk(&notes, &tree, temp_dir.path()).await?;

        // Verify the files exist and contain correct content
        for note in &notes {
            let file_path = temp_dir.path().join(format!("{}.md", note.id));
            let content = std::fs::read_to_string(&file_path)?;
            assert_eq!(content, note.content);
        }

        // Verify metadata.yaml exists and contains valid hierarchy
        let metadata_path = temp_dir.path().join("metadata.yaml");
        let metadata_content = std::fs::read_to_string(&metadata_path)?;

        let loaded_tree: Vec<SimpleNode> = serde_yaml::from_str(&metadata_content)?;

        // Find note1 in the tree and verify note2 is its child
        let root = loaded_tree.iter().find(|n| n.id == note1.id).unwrap();
        // Titles are now automatically set as H1 of content by Database
        // See commit 12acc9fb1b177b279181c4d15618e60571722ca1
        // assert_eq!(root.title, "Root Note");
        let child = &root.children[0];
        assert_eq!(child.id, note2.id);
        // assert_eq!(child.title, "Child Note");

        Ok(())
    }

    #[tokio::test]
    async fn test_read_from_disk() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a temporary directory
        let temp_dir = tempfile::tempdir()?;

        // Create test notes with specific content
        let note1 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Note 1".to_string(),
                content: "Content 1".to_string(),
            },
        )
        .await?;

        let note2 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Note 2".to_string(),
                content: "Content 2".to_string(),
            },
        )
        .await?;

        // Create hierarchy by attaching note2 as child of note1
        let attach_request = AttachChildRequest {
            child_note_id: note2.id,
            parent_note_id: Some(note1.id),
            hierarchy_type: Some("block".to_string()),
        };
        attach_child_note(base_url, attach_request).await?;

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Fetch current state
        let notes = fetch_notes(base_url, false).await?;
        let tree = fetch_note_tree(base_url).await?;

        // Write everything to disk
        write_notes_to_disk(&notes, &tree, temp_dir.path()).await?;

        // Modify the notes directly in database to verify our read_from_disk actually updates them
        let update1 = UpdateNoteRequest {
            title: Some("Changed Title 1".to_string()),
            content: "Changed Content 1".to_string(),
        };
        let update2 = UpdateNoteRequest {
            title: Some("Changed Title 2".to_string()),
            content: "Changed Content 2".to_string(),
        };
        update_note(base_url, note1.id, update1).await?;
        update_note(base_url, note2.id, update2).await?;

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Detach the notes to modify hierarchy
        detach_child_note(base_url, note2.id).await?;

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Now read everything back from disk and update the database
        match read_from_disk(base_url, temp_dir.path()).await {
            Ok(_) => {
                // Handle success case if needed
            }
            Err(e) => {
                eprintln!("Error reading from disk: {}", e);
                // Handle the error, e.g., log it or return it
            }
        }

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Verify the notes were restored to their original state
        let restored_notes = fetch_notes(base_url, false).await?;
        let restored_tree = fetch_note_tree(base_url).await?;

        // Verify note contents were restored
        let restored_note1 = restored_notes.iter().find(|n| n.id == note1.id).unwrap();
        let restored_note2 = restored_notes.iter().find(|n| n.id == note2.id).unwrap();
        assert_eq!(restored_note1.content, "Content 1");
        assert_eq!(restored_note2.content, "Content 2");

        // Verify hierarchy was restored
        let root = restored_tree.iter().find(|n| n.id == note1.id).unwrap();
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].id, note2.id);
        assert_eq!(root.children[0].hierarchy_type, Some("block".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_note_hash() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a test note
        let note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note".to_string(),
                content: "Test content".to_string(),
            },
        )
        .await?;

        // Get its hash
        let hash = get_note_hash(base_url, note.id).await?;
        assert!(!hash.is_empty(), "Hash should not be empty");

        // Try getting hash for non-existent note
        let result = get_note_hash(base_url, -1).await;
        assert!(matches!(result, Err(NoteError::NotFound(-1))));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_all_note_hashes() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create some test notes
        let note1 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Note 1".to_string(),
                content: "Content 1".to_string(),
            },
        )
        .await?;

        let note2 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Note 2".to_string(),
                content: "Content 2".to_string(),
            },
        )
        .await?;

        // Get all hashes
        let hashes = get_all_note_hashes(base_url).await?;

        // Verify our test notes' hashes are present
        let hash1 = hashes
            .iter()
            .find(|h| h.id == note1.id)
            .expect("Hash for note1 not found");
        let hash2 = hashes
            .iter()
            .find(|h| h.id == note2.id)
            .expect("Hash for note2 not found");
        assert!(!hash1.hash.is_empty());
        assert!(!hash2.hash.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_save_note_tree_as_yaml() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a test hierarchy
        let root_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Root Note".to_string(),
                content: "Root content".to_string(),
            },
        )
        .await?;

        let child_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child Note".to_string(),
                content: "Child content".to_string(),
            },
        )
        .await?;

        // Attach child to root
        let attach_request = AttachChildRequest {
            child_note_id: child_note.id,
            parent_note_id: Some(root_note.id),
            hierarchy_type: Some("block".to_string()),
        };
        attach_child_note(base_url, attach_request).await?;

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Fetch and save the tree
        let tree = fetch_note_tree(base_url).await?;
        let temp_file = std::env::temp_dir().join("note_tree.yaml");
        write_hierarchy_to_yaml(&tree, &temp_file)?;

        // Read back and verify
        let yaml_content = std::fs::read_to_string(&temp_file)?;

        let loaded_nodes: Vec<SimpleNode> = serde_yaml::from_str(&yaml_content)?;
        assert!(!loaded_nodes.is_empty(), "Tree should not be empty");

        // Find and verify root node
        let root = loaded_nodes
            .iter()
            .find(|n| n.id == root_note.id)
            .expect("Root note should exist in tree");
        assert!(!root.children.is_empty(), "Root should have children");

        // Verify child node
        let child = &root.children[0];
        assert_eq!(child.id, child_note.id, "Child ID should match");

        // Cleanup
        std::fs::remove_file(temp_file)?;

        Ok(())
    }
}
