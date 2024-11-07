use crate::api::compute_all_note_hashes;
pub use crate::api::tags::{AttachTagRequest, CreateTagRequest};
pub use crate::api::{
    compute_note_hash, AssetResponse, AttachChildRequest, BatchUpdateRequest, BatchUpdateResponse,
    CreateNoteRequest, ListAssetsParams, NoteHash, NoteTreeNode, TagResponse, UpdateAssetRequest,
    UpdateNoteRequest,
};
use crate::client::tags::{attach_tag_to_note, detach_tag_from_note};
pub use crate::tables::{HierarchyMapping, NoteWithParent, NoteWithoutFts};
use crate::{FLAT_API, SEARCH_FTS_API};
use futures::future::join_all;
use reqwest::Error as ReqwestError;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::fmt;
use tokio::fs;

// * Types ....................................................................
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct RenderedNote {
    pub id: i32,
    pub rendered_content: String,
}

#[derive(Debug)]
pub enum NoteError {
    NotFound(i32),
    RequestError(ReqwestError),
    IOError(std::io::Error),
    SerdeYamlError(serde_yaml::Error),
    SerdeJsonError(reqwest::Error),
    HttpStatusError(StatusCode),
    TagError(crate::client::tags::TagError),
}

impl fmt::Display for NoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteError::NotFound(id) => write!(f, "Note with id {} not found", id),
            NoteError::RequestError(e) => write!(f, "Request error: {}", e),
            NoteError::IOError(e) => write!(f, "IO error: {}", e),
            NoteError::SerdeYamlError(e) => write!(f, "YAML serialization error: {}", e),
            NoteError::SerdeJsonError(e) => write!(f, "JSON serialization error: {}", e),
            NoteError::HttpStatusError(code) => write!(f, "HTTP error with status code: {}", code),
            NoteError::TagError(e) => write!(f, "Tag error: {}", e),
        }
    }
}

impl std::error::Error for NoteError {}

impl From<ReqwestError> for NoteError {
    fn from(err: ReqwestError) -> Self {
        NoteError::RequestError(err)
    }
}

impl From<crate::client::tags::TagError> for NoteError {
    fn from(err: crate::client::tags::TagError) -> Self {
        NoteError::TagError(err)
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
// * Client ...................................................................
// ** Flat Functions ..........................................................
// *** Create .................................................................
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
// *** Read ...................................................................
// **** Single ................................................................

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
// **** All ...................................................................
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
// *** Update .................................................................
// **** Single ................................................................
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
// **** Batch .................................................................
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
        .await
        .map_err(NoteError::RequestError)?;

    if !response.status().is_success() {
        return Err(NoteError::HttpStatusError(response.status()));
    }

    let result = response.json::<BatchUpdateResponse>().await.map_err(|e| {
        if e.is_decode() {
            NoteError::SerdeJsonError(e)
        } else {
            NoteError::RequestError(e)
        }
    })?;
    Ok(result)
}
// *** Delete .................................................................
// ** Hierarchical Functions ..................................................
// *** Attach Child ...........................................................
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
// *** Detach Child ............................................................
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

// *** Get Tree ...............................................................
pub async fn fetch_note_tree(base_url: &str) -> Result<Vec<NoteTreeNode>, NoteError> {
    let url = format!("{}/notes/tree", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let note_tree = response.json::<Vec<NoteTreeNode>>().await?;
    Ok(note_tree)
}
// *** Get Mappings ...........................................................
pub async fn fetch_hierarchy_mappings(base_url: &str) -> Result<Vec<HierarchyMapping>, NoteError> {
    let url = format!("{}/notes/hierarchy", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let mappings = response.json::<Vec<HierarchyMapping>>().await?;
    Ok(mappings)
}
// ** Utils ...................................................................
// *** Sync to Disk ...........................................................
// **** Types and Utils .......................................................
#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq)]
pub struct SimpleNode {
    pub id: i32,
    #[allow(unused)]
    pub title: String,
    pub children: Vec<SimpleNode>,
}

fn simplify_tree(node: &NoteTreeNode) -> SimpleNode {
    SimpleNode {
        id: node.id,
        title: node.title.clone().expect("Node title should not be None"),
        children: node.children.iter().map(simplify_tree).collect(),
    }
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
        tags: vec![],
        children: simple_node
            .children
            .iter()
            .map(|child| simple_node_to_note_tree_node(child, content_map))
            .collect(),
    }
}
// **** Write .................................................................
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

    let simple_tree: Vec<SimpleNode> = tree.iter().map(simplify_tree).collect();

    // Save the simplified hierarchy as metadata.yaml
    let metadata_path = output_dir.join("metadata.yaml");
    let yaml = serde_yaml::to_string(&simple_tree)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    fs::write(metadata_path, yaml).await?;

    Ok(())
}
// **** Read ..................................................................
pub async fn read_from_disk(base_url: &str, input_dir: &std::path::Path) -> Result<(), NoteError> {
    // Read metadata.yaml to reconstruct the hierarchy
    let metadata_path = input_dir.join("metadata.yaml");
    let metadata_content = fs::read_to_string(&metadata_path).await?;
    let simple_nodes: Vec<SimpleNode> = serde_yaml::from_str(&metadata_content)?;

    // Flatten the tree to get all note IDs
    fn collect_note_ids(nodes: &[SimpleNode], ids: &mut Vec<i32>) {
        for node in nodes {
            ids.push(node.id);
            collect_note_ids(&node.children, ids);
        }
    }
    let mut note_ids = Vec::new();
    collect_note_ids(&simple_nodes, &mut note_ids);

    // Read note files concurrently
    let read_futures: Vec<_> = note_ids
        .iter()
        .map(|&id| {
            let file_path = input_dir.join(format!("{}.md", id));
            async move {
                let content = fs::read_to_string(&file_path).await?;
                Ok::<(i32, String), std::io::Error>((id, content))
            }
        })
        .collect();
    let note_contents = join_all(read_futures).await;
    let note_contents: Result<HashMap<i32, String>, _> = note_contents
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .map(|vec| vec.into_iter().collect());
    let note_contents = note_contents?;

    // Build a content map for reconstructing the note tree
    let content_map: HashMap<i32, String> = note_contents.clone();

    // Reconstruct local notes with hierarchy
    fn build_notes(
        node: &SimpleNode,
        parent_id: Option<i32>,
        contents: &HashMap<i32, String>,
        notes: &mut Vec<NoteWithParent>,
    ) {
        let content = contents.get(&node.id).cloned().unwrap_or_default();
        let note = NoteWithParent {
            note_id: node.id,
            title: node.title.clone(),
            content,
            created_at: None,
            modified_at: None,
            parent_id,
        };
        notes.push(note);
        for child in &node.children {
            build_notes(child, Some(node.id), contents, notes);
        }
    }
    let mut local_notes = Vec::new();
    for node in &simple_nodes {
        build_notes(node, None, &note_contents, &mut local_notes);
    }

    // Compute local hashes
    let local_hashes_map = compute_all_note_hashes(local_notes.clone()).await?;

    // Fetch remote hashes
    let remote_hashes = get_all_note_hashes(base_url).await?;
    let remote_hashes_map: HashMap<i32, String> = remote_hashes
        .iter()
        .map(|note_hash| (note_hash.id, note_hash.hash.clone()))
        .collect();

    // Identify notes that have changed
    let updates: Vec<_> = local_notes
        .into_iter()
        .filter_map(|note| {
            let local_hash = local_hashes_map.get(&note.note_id)?;
            let remote_hash = remote_hashes_map.get(&note.note_id);
            if Some(local_hash) != remote_hash {
                // Note has changed or is new
                let update_request = UpdateNoteRequest {
                    title: Some(note.title.clone()),
                    content: note.content.clone(),
                };
                Some((note.note_id, update_request))
            } else {
                None
            }
        })
        .collect();

    // Perform batch update for changed notes
    if !updates.is_empty() {
        batch_update_notes(base_url, updates).await?;
    }

    // Reconstruct the note tree from simple_nodes and content_map
    let note_tree_nodes: Vec<NoteTreeNode> = simple_nodes
        .iter()
        .map(|simple_node| simple_node_to_note_tree_node(simple_node, &content_map))
        .collect();

    // Update the note hierarchy in the database
    for note_tree_node in note_tree_nodes {
        update_note_tree(base_url, vec![note_tree_node]).await?;
    }

    Ok(())
}
// **** Json ..................................................................
// **** Files .................................................................
// *** Tree ...................................................................

pub async fn update_note_tree(base_url: &str, trees: Vec<NoteTreeNode>) -> Result<(), NoteError> {
    // First update the note content and structure
    let client = reqwest::Client::new();
    let url = format!("{}/notes/tree", base_url);
    client
        .put(url)
        .json(&trees)
        .send()
        .await?
        .error_for_status()
        .map_err(NoteError::from)?;

    // Give the server time to process the tree update
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Collect all nodes into a flat vector
    let mut nodes = Vec::new();
    for tree in trees {
        let mut stack = vec![tree];
        while let Some(node) = stack.pop() {
            nodes.push(node.clone());
            stack.extend(node.children.iter().cloned());
        }
    }

    Ok(())
}
// *** Hashes ....................................................................

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
// ** Search ..................................................................
// *** DB FTS .................................................................
pub async fn fts_search_notes(
    base_url: &str,
    query: &str,
) -> Result<Vec<NoteWithoutFts>, NoteError> {
    let url = format!(
        "{}/{SEARCH_FTS_API}?q={}",
        base_url,
        urlencoding::encode(query)
    );
    let response = reqwest::get(&url).await?.error_for_status()?;
    let notes = response.json::<Vec<NoteWithoutFts>>().await?;
    Ok(notes)
}
// *** Typesense ..............................................................
// **** Semantic ..............................................................
// **** TODO Hybrid ...........................................................
// **** TODO Direct ...........................................................
// **** TODO Semantic Similarity ........... ..................................
// ** Render ..................................................................
// *** Markdown ...............................................................
// **** Single .................................................................
/// Fetch rendered Markdown for a single note
pub async fn get_note_rendered_md(base_url: &str, note_id: i32) -> Result<String, NoteError> {
    let url = format!("{}/{FLAT_API}/{}/render/md", base_url, note_id);
    let response = reqwest::get(url).await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(note_id));
    }

    let response = response.error_for_status()?;
    let md = response.text().await?;
    Ok(md)
}
// **** All ....................................................................
/// Fetch rendered Markdown for all notes
pub async fn get_all_notes_rendered_md(base_url: &str) -> Result<Vec<RenderedNote>, NoteError> {
    let url = format!("{}/{FLAT_API}/render/md", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let rendered_notes = response.json::<Vec<RenderedNote>>().await?;
    Ok(rendered_notes)
}
// *** HTML ...................................................................
// **** Single .................................................................
/// Fetch rendered HTML for a single note
pub async fn get_note_rendered_html(base_url: &str, note_id: i32) -> Result<String, NoteError> {
    let url = format!("{}/{FLAT_API}/{}/render/html", base_url, note_id);
    let response = reqwest::get(url).await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(note_id));
    }

    let response = response.error_for_status()?;
    let html = response.text().await?;
    Ok(html)
}
// **** All ....................................................................
/// Fetch rendered HTML for all notes
pub async fn get_all_notes_rendered_html(base_url: &str) -> Result<Vec<RenderedNote>, NoteError> {
    let url = format!("{}/{FLAT_API}/render/html", base_url);
    let response = reqwest::get(url).await?.error_for_status()?;
    let rendered_notes = response.json::<Vec<RenderedNote>>().await?;
    Ok(rendered_notes)
}
// * Tests ....................................................................

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::tags::create_tag;
    use crate::BASE_URL;
    // ** Client ....................................................................
    // *** Functions .................................................................
    // **** Create ...................................................................
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
    // **** Read .....................................................................
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

    // **** Update ...................................................................
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
    // **** Delete ...................................................................
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
    // **** Tree .....................................................................
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

    use lazy_static::lazy_static;
    use std::sync::Mutex;

    lazy_static! {
        static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
    }

    #[tokio::test]
    async fn test_update_note_tree() {
        // Acquire mutex to ensure test runs in isolation
        let _lock = TEST_MUTEX.lock().unwrap();
        let base_url = BASE_URL;

        // Create test tags first
        let tag1 = crate::client::tags::create_tag(
            base_url,
            CreateTagRequest {
                name: "tag1".to_string(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let tag2 = crate::client::tags::create_tag(
            base_url,
            CreateTagRequest {
                name: "tag2".to_string(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let tag3 = crate::client::tags::create_tag(
            base_url,
            CreateTagRequest {
                name: "tag3".to_string(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Create root note with tag1
        let root_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Root Note".to_string(),
                content: "Root content".to_string(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        attach_tag_to_note(base_url, root_note.id, tag1.id)
            .await
            .unwrap_or_else(|e| panic!("Failed to attach tag1 to root note: {}", e));
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Create child notes with tags
        let child1_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child 1".to_string(),
                content: "Child 1 content".to_string(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        attach_tag_to_note(base_url, child1_note.id, tag2.id)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let child2_note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Child 2".to_string(),
                content: "Child 2 content".to_string(),
            },
        )
        .await
        .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        attach_tag_to_note(base_url, child2_note.id, tag3.id)
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Create a tree structure with updated tags
        let tree = NoteTreeNode {
            id: root_note.id,
            title: Some("Updated Root".to_string()),
            content: Some("Updated root content".to_string()),
            created_at: None,
            modified_at: None,
            tags: vec![tag1.clone(), tag2.clone()], // Add tag2 to root
            children: vec![
                NoteTreeNode {
                    id: child1_note.id,
                    title: Some("Updated Child 1".to_string()),
                    content: Some("Updated child 1 content".to_string()),
                    created_at: None,
                    modified_at: None,
                    tags: vec![tag2.clone(), tag3.clone()], // Add tag3 to child1
                    children: vec![],
                },
                NoteTreeNode {
                    id: child2_note.id,
                    title: Some("Updated Child 2".to_string()),
                    content: Some("Updated child 2 content".to_string()),
                    created_at: None,
                    modified_at: None,
                    tags: vec![tag3.clone(), tag1.clone()], // Add tag1 to child2
                    children: vec![],
                },
            ],
        };

        // Update the tree structure
        let update_result = update_note_tree(base_url, tree.clone()).await;
        // Test the result and print a useful error message
        if let Err(ref e) = update_result {
            eprintln!("Error updating note tree: {}", e);
            if let NoteError::RequestError(req_err) = e {
                if let Some(status) = req_err.status() {
                    eprintln!("Status code: {}", status);
                }
                if let Some(url) = req_err.url() {
                    eprintln!("URL: {}", url);
                }
            }
        }
        // If it failed to update, panic
        assert!(update_result.is_ok(), "Failed to update note tree");

        // Give the server time to process updates
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Fetch the updated tree
        let fetched_tree = fetch_note_tree(base_url).await.unwrap();

        // Find our test tree in the fetched trees
        let updated_tree = fetched_tree
            .iter()
            .find(|n| n.id == root_note.id)
            .expect("Could not find updated tree");

        // Verify the structure and content
        assert_eq!(
            updated_tree.content,
            Some("Updated root content".to_string())
        );
        assert_eq!(updated_tree.children.len(), 2);

        // Verify root tags
        assert!(
            updated_tree.tags.iter().any(|t| t.id == tag1.id),
            "Root should have tag1"
        );
        assert!(
            updated_tree.tags.iter().any(|t| t.id == tag2.id),
            "Root should have tag2"
        );

        // Verify children content and tags
        let child1 = updated_tree
            .children
            .iter()
            .find(|n| n.id == child1_note.id)
            .expect("Could not find child1");
        assert_eq!(child1.content, Some("Updated child 1 content".to_string()));
        assert!(
            child1.tags.iter().any(|t| t.id == tag2.id),
            "Child1 should have tag2"
        );
        assert!(
            child1.tags.iter().any(|t| t.id == tag3.id),
            "Child1 should have tag3"
        );

        let child2 = updated_tree
            .children
            .iter()
            .find(|n| n.id == child2_note.id)
            .expect("Could not find child2");
        assert_eq!(child2.content, Some("Updated child 2 content".to_string()));
        assert!(
            child2.tags.iter().any(|t| t.id == tag3.id),
            "Child2 should have tag3"
        );
        assert!(
            child2.tags.iter().any(|t| t.id == tag1.id),
            "Child2 should have tag1"
        );
    }
    // *** Utils .....................................................................

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

    #[tokio::test]
    async fn test_get_note_rendered_html() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a test note with markdown content
        let note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note".to_string(),
                content: "**Bold** and *italic*".to_string(),
            },
        )
        .await?;

        // Test single note HTML rendering
        let html = get_note_rendered_html(base_url, note.id).await?;
        assert!(html.contains("<strong>Bold</strong>"));
        assert!(html.contains("<em>italic</em>"));

        // Test non-existent note
        let result = get_note_rendered_html(base_url, -1).await;
        assert!(matches!(result, Err(NoteError::NotFound(-1))));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_note_rendered_md() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a test note with markdown content
        let note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note".to_string(),
                content: "λ#(21*2)#".to_string(),
            },
        )
        .await?;

        // Test single note Markdown rendering
        let md = get_note_rendered_md(base_url, note.id).await?;
        dbg!(format!("in: {}", note.content));
        dbg!(format!("MD: {}", md));
        dbg!(md.contains("42"));
        assert!(md.contains("42")); // Assuming Rhai block was executed

        // Test non-existent note
        let result = get_note_rendered_md(base_url, -1).await;
        assert!(matches!(result, Err(NoteError::NotFound(-1))));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_all_notes_rendered_html() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create multiple test notes
        let note1 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note 1".to_string(),
                content: "**Bold** text".to_string(),
            },
        )
        .await?;

        let note2 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note 2".to_string(),
                content: "*Italic* text".to_string(),
            },
        )
        .await?;

        // Test all notes HTML rendering
        let rendered_notes = get_all_notes_rendered_html(base_url).await?;
        assert!(!rendered_notes.is_empty());

        // Find our test notes in the results
        let rendered1 = rendered_notes.iter().find(|n| n.id == note1.id).unwrap();
        let rendered2 = rendered_notes.iter().find(|n| n.id == note2.id).unwrap();

        assert!(rendered1.rendered_content.contains("<strong>Bold</strong>"));
        assert!(rendered2.rendered_content.contains("<em>Italic</em>"));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_all_notes_rendered_md() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create multiple test notes
        let note1 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note 1".to_string(),
                content: "λ#(21*2)#".to_string(),
            },
        )
        .await?;

        let note2 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note 2".to_string(),
                content: "Regular **markdown**".to_string(),
            },
        )
        .await?;

        // Test all notes Markdown rendering
        let rendered_notes = get_all_notes_rendered_md(base_url).await?;
        assert!(!rendered_notes.is_empty());

        // Find our test notes in the results
        let rendered1 = rendered_notes.iter().find(|n| n.id == note1.id).unwrap();
        let rendered2 = rendered_notes.iter().find(|n| n.id == note2.id).unwrap();

        assert!(rendered1.rendered_content.contains("42")); // Rhai block executed
        assert!(rendered2.rendered_content.contains("**markdown**")); // Regular markdown preserved

        Ok(())
    }

    #[tokio::test]
    async fn test_get_all_note_hashes() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create a few test notes first
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

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Get all note hashes
        let hashes = get_all_note_hashes(base_url).await?;

        // Verify we got hashes
        assert!(!hashes.is_empty(), "Should have received some note hashes");

        // Find our test notes' hashes
        let hash1 = hashes.iter().find(|h| h.id == note1.id);
        let hash2 = hashes.iter().find(|h| h.id == note2.id);

        assert!(hash1.is_some(), "Hash for note1 should exist");
        assert!(hash2.is_some(), "Hash for note2 should exist");

        let hash1 = hash1.unwrap();
        let hash2 = hash2.unwrap();

        // Verify hash format (should be non-empty strings)
        assert!(!hash1.hash.is_empty(), "Hash1 should not be empty");
        assert!(!hash2.hash.is_empty(), "Hash2 should not be empty");

        // Verify hashes are different for different notes
        assert_ne!(
            hash1.hash, hash2.hash,
            "Different notes should have different hashes"
        );

        // Modify a note and verify its hash changes
        let update = UpdateNoteRequest {
            title: Some("Modified Note 1".to_string()),
            content: "Modified Content 1".to_string(),
        };
        update_note(base_url, note1.id, update).await?;

        // Give the server time to process
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Get updated hashes
        let updated_hashes = get_all_note_hashes(base_url).await?;
        let updated_hash1 = updated_hashes
            .iter()
            .find(|h| h.id == note1.id)
            .expect("Modified note should still exist");

        assert_ne!(
            hash1.hash, updated_hash1.hash,
            "Hash should change when note content changes"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_fts_search_notes() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // Create test notes with specific content
        let note1 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note 1".to_string(),
                content: "The quick brown fox jumps over the lazy dog".to_string(),
            },
        )
        .await?;

        let note2 = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note 2".to_string(),
                content: "Pack my box with five dozen liquor jugs".to_string(),
            },
        )
        .await?;

        // Give the database time to update the FTS index
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Test exact word match
        let results = fts_search_notes(base_url, "fox").await?;
        assert!(!results.is_empty());
        assert!(results.iter().any(|n| n.id == note1.id));
        assert!(!results.iter().any(|n| n.id == note2.id));

        // Test partial word match (should not match due to FTS tokenization)
        let results = fts_search_notes(base_url, "fo").await?;
        assert!(results.is_empty());

        // Test multiple word search
        let results = fts_search_notes(base_url, "quick brown").await?;
        assert!(!results.is_empty());
        assert!(results.iter().any(|n| n.id == note1.id));

        // Test stemming (searching for "jumping" should find "jumps")
        let results = fts_search_notes(base_url, "jumping").await?;
        assert!(!results.is_empty());
        assert!(results.iter().any(|n| n.id == note1.id));

        // Test stop word handling ("the" should be ignored)
        let results = fts_search_notes(base_url, "the").await?;
        assert!(results.is_empty());

        // Test non-existent term
        let results = fts_search_notes(base_url, "nonexistentterm").await?;
        assert!(results.is_empty());

        Ok(())
    }
}
