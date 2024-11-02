use crate::{api::NoteResponse, FLAT_API};
use reqwest::Error as ReqwestError;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug)]
pub enum NoteError {
    NotFound(i32),
    RequestError(ReqwestError),
}

impl fmt::Display for NoteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoteError::NotFound(id) => write!(f, "Note with id {} not found", id),
            NoteError::RequestError(e) => write!(f, "Request error: {}", e),
        }
    }
}

impl std::error::Error for NoteError {}

impl From<ReqwestError> for NoteError {
    fn from(err: ReqwestError) -> Self {
        NoteError::RequestError(err)
    }
}

#[derive(Serialize)]
pub struct CreateNoteRequest {
    pub title: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct UpdateNoteRequest {
    pub title: String,
    pub content: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct HierarchyMapping {
    pub child_id: i32,
    pub parent_id: Option<i32>,
    pub hierarchy_type: Option<String>,
}

#[derive(Serialize)]
pub struct AttachChildRequest {
    pub child_note_id: i32,
    pub parent_note_id: Option<i32>,
    pub hierarchy_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct NoteTreeNode {
    pub id: i32,
    pub title: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub hierarchy_type: Option<String>,
    pub children: Vec<NoteTreeNode>,
}

pub async fn fetch_note(
    base_url: &str,
    id: i32,
    metadata_only: bool,
) -> Result<NoteResponse, NoteError> {
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
    let note = response.json::<NoteResponse>().await?;

    // If metadata_only is true, ensure content field is empty
    if metadata_only {
        Ok(NoteResponse {
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
) -> Result<Vec<NoteResponse>, NoteError> {
    let url = if metadata_only {
        format!("{}/{FLAT_API}?metadata_only=true", base_url)
    } else {
        format!("{}/{FLAT_API}", base_url)
    };
    let response = reqwest::get(url).await?.error_for_status()?;
    let notes = response.json::<Vec<NoteResponse>>().await?;

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
) -> Result<NoteResponse, NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}", base_url);
    let response = client
        .post(url)
        .json(&note)
        .send()
        .await?
        .error_for_status()?;
    let created_note = response.json::<NoteResponse>().await?;
    Ok(created_note)
}

pub async fn update_note(
    base_url: &str,
    id: i32,
    note: UpdateNoteRequest,
) -> Result<NoteResponse, NoteError> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}/{}", base_url, id);
    let response = client.put(url).json(&note).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(NoteError::NotFound(id));
    }

    let response = response.error_for_status()?;
    let updated_note = response.json::<NoteResponse>().await?;
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
            title: "Updated Test Note".to_string(),
            content: "This is an updated test note".to_string(),
        };
        let result = update_note(base_url, created_note.id, update_note_req).await;
        assert!(result.is_ok());
        let updated_note = result.unwrap();
        assert_eq!(updated_note.id, created_note.id);
        assert_eq!(updated_note.title, "Updated Test Note");
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
}
