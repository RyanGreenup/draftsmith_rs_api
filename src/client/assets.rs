pub use crate::api::{
    compute_note_hash, AssetResponse, AttachChildRequest, BatchUpdateRequest, BatchUpdateResponse,
    CreateNoteRequest, ListAssetsParams, NoteHash, NoteTreeNode, UpdateAssetRequest,
    UpdateNoteRequest,
};
pub use crate::tables::{HierarchyMapping, NoteWithParent, NoteWithoutFts};
use std::fmt;

// * Types ....................................................................

#[derive(Debug)]
pub enum AssetError {
    NotFound(i32),
    FileNotFound(String),
    RequestError(reqwest::Error),
    IOError(std::io::Error),
}

impl fmt::Display for AssetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AssetError::NotFound(id) => write!(f, "Asset with id {} not found", id),
            AssetError::FileNotFound(file_path) => {
                write!(f, "Asset file '{}' not found", file_path)
            }
            AssetError::RequestError(e) => write!(f, "Request error: {}", e),
            AssetError::IOError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AssetError {}

impl From<reqwest::Error> for AssetError {
    fn from(err: reqwest::Error) -> Self {
        AssetError::RequestError(err)
    }
}

impl From<std::io::Error> for AssetError {
    fn from(err: std::io::Error) -> Self {
        AssetError::IOError(err)
    }
}

// * Client Bindings ............................................................
// ** Create ...................................................................
pub async fn create_asset(
    base_url: &str,
    file_path: &std::path::Path,
    note_id: Option<i32>,
    description: Option<String>,
    filename: Option<String>,
) -> Result<AssetResponse, AssetError> {
    let client = reqwest::Client::new();
    let url = format!("{}/assets", base_url);

    // Create multipart form
    let mut form = reqwest::multipart::Form::new();

    // Add file
    let file_content = tokio::fs::read(file_path).await?;
    let file_part = reqwest::multipart::Part::bytes(file_content).file_name(
        filename.clone().unwrap_or_else(|| {
            file_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string()
        }),
    );
    form = form.part("file", file_part);

    // Add note_id if provided
    if let Some(id) = note_id {
        form = form.text("note_id", id.to_string());
    }

    // Add description if provided
    if let Some(desc) = description {
        form = form.text("description", desc);
    }

    // Add custom filename if provided
    if let Some(name) = filename {
        form = form.text("filename", name);
    }

    // Send request
    let response = client.post(url).multipart(form).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AssetError::NotFound(-1));
    }

    let asset = response.error_for_status()?.json::<AssetResponse>().await?;
    Ok(asset)
}

// ** Read .....................................................................
// *** List ....................................................................

pub async fn list_assets(
    base_url: &str,
    note_id: Option<i32>,
) -> Result<Vec<AssetResponse>, AssetError> {
    let client = reqwest::Client::new();
    let mut url = format!("{}/assets", base_url);

    // Add query parameters if note_id is provided
    if let Some(id) = note_id {
        url = format!("{}?note_id={}", url, id);
    }

    let response = client.get(&url).send().await?.error_for_status()?;
    let assets = response.json::<Vec<AssetResponse>>().await?;
    Ok(assets)
}

// **** Download ................................................................
// ***** Id ......................................................................

pub async fn get_asset(
    base_url: &str,
    asset_id: i32,
    output_path: &std::path::Path,
) -> Result<(), AssetError> {
    let client = reqwest::Client::new();
    let url = format!("{}/assets/{}", base_url, asset_id);

    let response = client.get(&url).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AssetError::NotFound(asset_id));
    }

    // Get the response bytes and write them directly to the file
    let bytes = response.error_for_status()?.bytes().await?;
    tokio::fs::write(output_path, bytes).await?;

    Ok(())
}

// ***** Name ....................................................................
pub async fn get_asset_by_name(
    base_url: &str,
    asset_name: &str,
    output_path: &std::path::Path,
) -> Result<(), AssetError> {
    let client = reqwest::Client::new();
    let url = format!("{}/assets/download/{}", base_url, asset_name);

    let response = client.get(&url).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AssetError::FileNotFound(String::from(asset_name)));
    }

    // Get the response bytes
    let bytes = response.error_for_status()?.bytes().await?;

    // If output_path is a directory, construct the full path using the asset name
    let final_path = if output_path.is_dir() {
        // Extract the filename from asset_name (last component of the path)
        let filename = std::path::Path::new(asset_name)
            .file_name()
            .ok_or_else(|| AssetError::FileNotFound("Invalid asset name".to_string()))?;
        output_path.join(filename)
    } else {
        output_path.to_path_buf()
    };

    // Create parent directories if they don't exist
    if let Some(parent) = final_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    // Write the bytes to the final path
    tokio::fs::write(&final_path, bytes).await?;

    Ok(())
}

// ** Update ...................................................................
pub async fn update_asset(
    base_url: &str,
    asset_id: i32,
    payload: UpdateAssetRequest,
) -> Result<AssetResponse, AssetError> {
    let client = reqwest::Client::new();
    let url = format!("{}/assets/{}", base_url, asset_id);

    let response = client.put(url).json(&payload).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AssetError::NotFound(asset_id));
    }

    let asset = response.error_for_status()?.json::<AssetResponse>().await?;
    Ok(asset)
}

// ** Delete ...................................................................
pub async fn delete_asset(base_url: &str, asset_id: i32) -> Result<(), AssetError> {
    let client = reqwest::Client::new();
    let url = format!("{}/assets/{}", base_url, asset_id);

    let response = client.delete(url).send().await?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AssetError::NotFound(asset_id));
    }

    response.error_for_status()?;
    Ok(())
}

// * Tests ....................................................................
#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{create_note, delete_note};
    use crate::BASE_URL;
    use std::io::Write;

    // ** Create ...................................................................
    #[tokio::test]
    async fn test_create_asset_with_options() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = crate::BASE_URL;

        // Create a temporary file with test content
        let mut temp_file = tempfile::NamedTempFile::new()?;
        write!(temp_file, "test content with options")?;

        // Test case 1: Basic asset creation with just file
        let created_asset1 = create_asset(base_url, temp_file.path(), None, None, None).await?;

        assert!(created_asset1.id > 0);
        assert!(created_asset1.location.exists());
        assert_eq!(created_asset1.note_id, None);
        assert_eq!(created_asset1.description, None);

        // Test case 2: Asset creation with all optional parameters
        let description = Some("Test asset with full options".to_string());
        let custom_filename = Some("custom_test_file.txt".to_string());

        // First create a note to link the asset to
        let note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note for Asset".to_string(),
                content: "Test content".to_string(),
            },
        )
        .await?;

        let created_asset2 = create_asset(
            base_url,
            temp_file.path(),
            Some(note.id),
            description.clone(),
            custom_filename.clone(),
        )
        .await?;

        assert!(created_asset2.id > 0);
        assert!(created_asset2.location.exists());
        assert_eq!(created_asset2.note_id, Some(note.id));
        assert_eq!(created_asset2.description, description);
        assert!(created_asset2
            .location
            .to_string_lossy()
            .contains("custom_test_file.txt"));

        // Test case 3: Create asset with invalid note_id
        let result = create_asset(base_url, temp_file.path(), Some(-1), None, None).await;

        assert!(matches!(result, Err(AssetError::RequestError(_))));

        // Cleanup
        // Delete the assets
        let _ = delete_asset(base_url, created_asset1.id).await;
        let _ = delete_asset(base_url, created_asset2.id).await;
        // Delete the note
        let _ = delete_note(base_url, note.id).await;

        Ok(())
    }

    // ** Read .....................................................................
    // *** List ....................................................................

    #[tokio::test]
    async fn test_list_assets() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // First create an asset to ensure we have something to list
        let mut temp_file = tempfile::NamedTempFile::new()?;
        write!(temp_file, "test content")?;

        let created_asset = create_asset(
            base_url,
            temp_file.path(),
            None,
            Some("Test asset for listing".to_string()),
            None,
        )
        .await?;

        // List all assets
        let assets = list_assets(base_url, None).await?;

        // Verify we can find our created asset
        assert!(!assets.is_empty());
        let found_asset = assets.iter().find(|a| a.id == created_asset.id);
        assert!(found_asset.is_some());
        let found_asset = found_asset.unwrap();
        assert_eq!(
            found_asset.description,
            Some("Test asset for listing".to_string())
        );

        // Test filtering by note_id
        let filtered_assets = list_assets(base_url, Some(999999)).await?; // Using a likely non-existent note_id
        assert!(filtered_assets.is_empty());

        Ok(())
    }

    // *** Download ................................................................
    // **** Id ......................................................................

    #[tokio::test]
    async fn test_get_asset() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = crate::BASE_URL;

        // First create a test file
        let mut temp_file = tempfile::NamedTempFile::new()?;
        write!(temp_file, "test content")?;

        // Create an asset with the test file
        let created_asset = create_asset(
            base_url,
            temp_file.path(),
            None,
            Some("Test asset".to_string()),
            None,
        )
        .await?;

        // Create a temporary file for the downloaded content
        let output_path = std::env::temp_dir().join("test_download.tmp");

        // Get the asset's content
        get_asset(base_url, created_asset.id, &output_path).await?;

        // Read and verify the content matches what we uploaded
        let downloaded_content = std::fs::read(&output_path)?;
        assert_eq!(downloaded_content, b"test content");

        // Clean up the temporary file
        std::fs::remove_file(&output_path)?;

        // Test getting a non-existent asset
        let bad_output_path = std::env::temp_dir().join("nonexistent.tmp");
        let result = get_asset(base_url, -1, &bad_output_path).await;
        assert!(matches!(result, Err(AssetError::NotFound(-1))));

        Ok(())
    }

    // **** Name ....................................................................
    #[tokio::test]
    async fn test_get_asset_by_name() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = crate::BASE_URL;

        // First create a test file with known content
        let mut temp_file = tempfile::NamedTempFile::new()?;
        write!(temp_file, "test content for filename download")?;

        // Create an asset with a specific filename
        let custom_filename = "foo/bar/baz/test_download_by_name.txt".to_string();
        let created_asset = create_asset(
            base_url,
            temp_file.path(),
            None,
            Some("Test asset for filename download".to_string()),
            Some(custom_filename.clone()),
        )
        .await?;

        // Create a temporary file for the downloaded content
        let output_path = std::env::temp_dir().join("test_download_by_name_output.tmp");

        // Get the asset by filename
        get_asset_by_name(base_url, &custom_filename, &output_path).await?;

        // Read and verify the content matches what we uploaded
        let downloaded_content = std::fs::read_to_string(&output_path)?;
        assert_eq!(downloaded_content, "test content for filename download");

        // Clean up
        std::fs::remove_file(&output_path)?;
        delete_asset(base_url, created_asset.id).await?;

        // Test getting a non-existent asset
        let bad_output_path = std::env::temp_dir().join("nonexistent.tmp");
        let result = get_asset_by_name(base_url, "nonexistent.txt", &bad_output_path).await;
        assert!(matches!(result, Err(AssetError::FileNotFound(_))));

        Ok(())
    }

    // ** Update ...................................................................
    #[tokio::test]
    async fn test_update_asset() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // First create a test file and asset
        let mut temp_file = tempfile::NamedTempFile::new()?;
        write!(temp_file, "test content")?;

        let created_asset = create_asset(
            base_url,
            temp_file.path(),
            None,
            Some("Initial description".to_string()),
            None,
        )
        .await?;

        // Create a test note to link the asset to
        let note = create_note(
            base_url,
            CreateNoteRequest {
                title: "Test Note for Asset Update".to_string(),
                content: "Test content".to_string(),
            },
        )
        .await?;

        // Update the asset
        let update_payload = UpdateAssetRequest {
            note_id: Some(note.id),
            description: Some("Updated description".to_string()),
        };

        let updated_asset = update_asset(base_url, created_asset.id, update_payload).await?;

        // Verify the updates
        assert_eq!(updated_asset.id, created_asset.id);
        assert_eq!(updated_asset.note_id, Some(note.id));
        assert_eq!(
            updated_asset.description,
            Some("Updated description".to_string())
        );

        // Test updating non-existent asset
        let bad_payload = UpdateAssetRequest {
            note_id: None,
            description: None,
        };
        let result = update_asset(base_url, -1, bad_payload).await;
        assert!(matches!(result, Err(AssetError::NotFound(-1))));

        // Cleanup
        delete_asset(base_url, created_asset.id).await?;
        delete_note(base_url, note.id).await?;

        Ok(())
    }

    // ** Delete ...................................................................
    #[tokio::test]
    async fn test_delete_asset() -> Result<(), Box<dyn std::error::Error>> {
        let base_url = BASE_URL;

        // First create a test file and asset
        let mut temp_file = tempfile::NamedTempFile::new()?;
        write!(temp_file, "test content")?;

        let created_asset = create_asset(
            base_url,
            temp_file.path(),
            None,
            Some("Test asset for deletion".to_string()),
            None,
        )
        .await?;

        // Delete the asset
        delete_asset(base_url, created_asset.id).await?;

        // Verify the asset was deleted by trying to get it
        let output_path = std::env::temp_dir().join("deleted_asset_test.tmp");
        let result = get_asset(base_url, created_asset.id, &output_path).await;
        assert!(matches!(result, Err(AssetError::NotFound(_))));

        // Test deleting non-existent asset
        let result = delete_asset(base_url, -1).await;
        assert!(matches!(result, Err(AssetError::NotFound(-1))));

        Ok(())
    }
}
