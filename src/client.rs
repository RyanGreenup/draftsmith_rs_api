use crate::{api::NoteResponse, FLAT_API, BASE_URL};
use reqwest::Error;
use serde::Serialize;

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

pub async fn fetch_note(base_url: &str, id: i32, metadata_only: bool) -> Result<NoteResponse, Error> {
    let url = if metadata_only {
        format!("{}/{FLAT_API}/{}?exclude_content=true", base_url, id)
    } else {
        format!("{}/{FLAT_API}/{}", base_url, id)
    };
    let response = reqwest::get(url).await?;
    let note = response.json::<NoteResponse>().await?;
    Ok(note)
}

pub async fn fetch_notes(base_url: &str, metadata_only: bool) -> Result<Vec<NoteResponse>, Error> {
    let url = if metadata_only {
        format!("{}/{FLAT_API}?exclude_content=true", base_url)
    } else {
        format!("{}/{FLAT_API}", base_url)
    };
    let response = reqwest::get(url).await?;
    let notes = response.json::<Vec<NoteResponse>>().await?;
    Ok(notes)
}

pub async fn create_note(base_url: &str, note: CreateNoteRequest) -> Result<NoteResponse, Error> {
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
) -> Result<NoteResponse, Error> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}/{}", base_url, id);
    let response = client
        .put(url)
        .json(&note)
        .send()
        .await?
        .error_for_status()?;
    let updated_note = response.json::<NoteResponse>().await?;
    Ok(updated_note)
}

pub async fn delete_note(base_url: &str, id: i32) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}/{}", base_url, id);
    client.delete(url).send().await?.error_for_status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(fetch_result.is_err());
    }
}
