use reqwest::{self, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TagError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Tag not found")]
    NotFound,

    #[error("Unexpected server error: {0}")]
    ServerError(String),
}

// * Types ....................................................................

#[derive(Serialize)]
pub struct CreateTagRequest {
    pub name: String,
}

#[derive(Deserialize)]
pub struct TagResponse {
    pub id: i32,
    pub name: String,
}

#[derive(Serialize)]
pub struct UpdateTagRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct AttachChildTagRequest {
    pub parent_id: i32,
    pub child_id: i32,
}
// * Client ...................................................................
// ** Flat Functions ..........................................................
// *** Create .................................................................

pub async fn create_tag(base_url: &str, tag: CreateTagRequest) -> Result<TagResponse, TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags", base_url);
    let response = client
        .post(&url)
        .json(&tag)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    let tag_response = response
        .json::<TagResponse>()
        .await
        .map_err(TagError::NetworkError)?;
    Ok(tag_response)
}
// *** Read ...................................................................
// **** Get Tag ...............................................................
pub async fn get_tag(base_url: &str, id: i32) -> Result<TagResponse, TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags/{}", base_url, id);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    let tag = response
        .json::<TagResponse>()
        .await
        .map_err(TagError::NetworkError)?;
    Ok(tag)
}

// **** List Tags .............................................................

pub async fn list_tags(base_url: &str) -> Result<Vec<TagResponse>, TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags", base_url);

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    let tags = response
        .json::<Vec<TagResponse>>()
        .await
        .map_err(TagError::NetworkError)?;
    Ok(tags)
}
// *** Update .................................................................
pub async fn update_tag(
    base_url: &str,
    id: i32,
    update: UpdateTagRequest,
) -> Result<TagResponse, TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags/{}", base_url, id);

    let response = client
        .put(&url)
        .json(&update)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    let updated_tag = response
        .json::<TagResponse>()
        .await
        .map_err(TagError::NetworkError)?;
    Ok(updated_tag)
}
// *** Delete .................................................................
pub async fn delete_tag(base_url: &str, id: i32) -> Result<(), TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags/{}", base_url, id);

    let response = client
        .delete(&url)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    Ok(())
}

// ** Hierarchical Functions ..................................................
// *** Attach Child ...........................................................
pub async fn attach_child_tag(
    base_url: &str,
    parent_id: i32,
    child_id: i32,
) -> Result<(), TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags/attach", base_url);

    let request = AttachChildTagRequest {
        parent_id,
        child_id,
    };

    let response = client
        .post(&url)
        .json(&request)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    Ok(())
}

// *** Detach Child ...........................................................
pub async fn detach_child_tag(base_url: &str, child_id: i32) -> Result<(), TagError> {
    let client = reqwest::Client::new();
    let url = format!("{}/tags/hierarchy/detach/{}", base_url, child_id);

    let response = client
        .delete(&url)
        .send()
        .await
        .map_err(TagError::NetworkError)?;

    if response.status() == StatusCode::NOT_FOUND {
        return Err(TagError::NotFound);
    }

    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(TagError::ServerError(error_text));
    }

    Ok(())
}

// *** Get Tree ...............................................................
// *** Get Mappings ...........................................................
#[cfg(test)]
mod tests {
    // * Tests  ...................................................................
    // ** Client ..............................................................
    use super::*;
    use crate::BASE_URL;
    use tokio;
    // *** Functions ..........................................................
    // **** Create ............................................................

    #[tokio::test]
    async fn test_create_tag() {
        let base_url = BASE_URL;
        let new_tag = CreateTagRequest {
            name: "Test Tag".to_string(),
        };

        match create_tag(base_url, new_tag).await {
            Ok(tag_response) => {
                assert_eq!(tag_response.name, "Test Tag");
            }
            Err(TagError::NetworkError(e)) => {
                panic!("Network error occurred: {:?}", e);
            }
            Err(TagError::ServerError(e)) => {
                panic!("Server error occurred: {:?}", e);
            }
            Err(e) => {
                panic!("An error occurred: {:?}", e);
            }
        }
    }

    // **** Read ..............................................................
    #[tokio::test]
    async fn test_get_tag() {
        let base_url = BASE_URL;

        // First create a tag to retrieve
        let new_tag = CreateTagRequest {
            name: "Test Tag for Get".to_string(),
        };

        // Create a new tag and get its ID
        let created_tag = create_tag(base_url, new_tag)
            .await
            .expect("Failed to create tag for testing get");

        // Test getting the tag
        let retrieved_tag = get_tag(base_url, created_tag.id)
            .await
            .expect("Failed to retrieve tag");

        // Verify the retrieved tag matches the created tag
        assert_eq!(retrieved_tag.id, created_tag.id);
        assert_eq!(retrieved_tag.name, created_tag.name);

        // Test getting a non-existent tag
        let non_existent_result = get_tag(base_url, 99999).await;
        assert!(matches!(non_existent_result, Err(TagError::NotFound)));
    }

    #[tokio::test]
    async fn test_list_tags() {
        let base_url = BASE_URL;
        let new_tag = CreateTagRequest {
            name: "Test Tag for List".to_string(),
        };

        // Create a new tag to ensure there is data to retrieve
        let created_tag = create_tag(base_url, new_tag)
            .await
            .expect("Failed to create tag for testing list");

        // Call the list_tags function
        let tags_list = list_tags(base_url)
            .await
            .expect("Failed to retrieve tags list");

        // Check that the list contains the newly created tag
        assert!(
            tags_list.iter().any(|tag| tag.id == created_tag.id),
            "Created tag not found in tags list"
        );
    }

    // **** Update ............................................................
    #[tokio::test]
    async fn test_update_tag() {
        let base_url = BASE_URL;

        // First create a tag to update
        let new_tag = CreateTagRequest {
            name: "Test Tag for Update".to_string(),
        };

        // Create a new tag
        let created_tag = create_tag(base_url, new_tag)
            .await
            .expect("Failed to create tag for testing update");

        // Update the tag
        let update = UpdateTagRequest {
            name: "Updated Test Tag".to_string(),
        };

        let updated_tag = update_tag(base_url, created_tag.id, update)
            .await
            .expect("Failed to update tag");

        // Verify the update was successful
        assert_eq!(updated_tag.id, created_tag.id);
        assert_eq!(updated_tag.name, "Updated Test Tag");

        // Test updating a non-existent tag
        let non_existent_update = UpdateTagRequest {
            name: "This should fail".to_string(),
        };
        let non_existent_result = update_tag(base_url, 99999, non_existent_update).await;
        assert!(matches!(non_existent_result, Err(TagError::NotFound)));
    }

    // **** Delete ............................................................
    #[tokio::test]
    async fn test_delete_tag() {
        let base_url = BASE_URL;

        // First create a tag to delete
        let new_tag = CreateTagRequest {
            name: "Test Tag for Delete".to_string(),
        };

        // Create a new tag
        let created_tag = create_tag(base_url, new_tag)
            .await
            .expect("Failed to create tag for testing delete");

        // Delete the tag
        delete_tag(base_url, created_tag.id)
            .await
            .expect("Failed to delete tag");

        // Verify the tag was deleted by attempting to get it
        let get_result = get_tag(base_url, created_tag.id).await;
        assert!(matches!(get_result, Err(TagError::NotFound)));

        // Test deleting a non-existent tag
        let non_existent_result = delete_tag(base_url, 99999).await;
        assert!(matches!(non_existent_result, Err(TagError::NotFound)));
    }

    #[tokio::test]
    async fn test_detach_child_tag() {
        let base_url = BASE_URL;

        // Create parent tag
        let parent_tag = create_tag(
            base_url,
            CreateTagRequest {
                name: "Parent Tag for Detach".to_string(),
            },
        )
        .await
        .expect("Failed to create parent tag");

        // Create child tag
        let child_tag = create_tag(
            base_url,
            CreateTagRequest {
                name: "Child Tag for Detach".to_string(),
            },
        )
        .await
        .expect("Failed to create child tag");

        // First attach the child tag
        attach_child_tag(base_url, parent_tag.id, child_tag.id)
            .await
            .expect("Failed to attach child tag");

        // Test detaching the child tag
        detach_child_tag(base_url, child_tag.id)
            .await
            .expect("Failed to detach child tag");

        // Test detaching a non-existent tag
        let non_existent_result = detach_child_tag(base_url, 99999).await;
        assert!(matches!(non_existent_result, Err(TagError::NotFound)));
    }

    #[tokio::test]
    async fn test_attach_child_tag() {
        let base_url = BASE_URL;

        // Create parent tag
        let parent_tag = create_tag(
            base_url,
            CreateTagRequest {
                name: "Parent Tag".to_string(),
            },
        )
        .await
        .expect("Failed to create parent tag");

        // Create child tag
        let child_tag = create_tag(
            base_url,
            CreateTagRequest {
                name: "Child Tag".to_string(),
            },
        )
        .await
        .expect("Failed to create child tag");

        // Test attaching child to parent
        attach_child_tag(base_url, parent_tag.id, child_tag.id)
            .await
            .expect("Failed to attach child tag");

        // Test attaching to non-existent parent
        let non_existent_result = attach_child_tag(base_url, 99999, child_tag.id).await;
        assert!(matches!(non_existent_result, Err(TagError::NotFound)));

        // Test attaching non-existent child
        let non_existent_result = attach_child_tag(base_url, parent_tag.id, 99999).await;
        assert!(matches!(non_existent_result, Err(TagError::NotFound)));
    }
}
// **** Tree ..............................................................
// *** Utils ..............................................................
