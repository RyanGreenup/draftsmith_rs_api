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
// *** Delete .................................................................
// ** Hierarchical Functions ..................................................
// *** Attach Child ...........................................................
// *** Detach Child ...........................................................
// *** Get Tree ...............................................................
// *** Get Mappings ...........................................................
#[cfg(test)]
mod tests {
    // * Tests  ...................................................................
    // ** Client ..............................................................
    use super::*;
    use crate::BASE_URL;
    use tokio;

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
}
// *** Functions ..........................................................
// **** Create ............................................................
// **** Read ..............................................................
// **** Update ............................................................
// **** Delete ............................................................
// **** Tree ..............................................................
// *** Utils ..............................................................
