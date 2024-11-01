use reqwest::Error;
use serde::Serialize;
use crate::api::NoteResponse;
use crate::BASE_URL;

const FLAT_API: &str = "notes/flat";

#[derive(Serialize)]
pub struct CreateNoteRequest {
    pub title: String,
    pub content: String
}

pub async fn fetch_note(base_url: &str, id: i32) -> Result<NoteResponse, Error> {
    let url = format!("{}/{FLAT_API}/{}", base_url, id);
    let response = reqwest::get(url).await?;
    let note = response.json::<NoteResponse>().await?;
    Ok(note)
}

pub async fn fetch_notes(base_url: &str) -> Result<Vec<NoteResponse>, Error> {
    let url = format!("{}/{FLAT_API}", base_url);
    let response = reqwest::get(url).await?;
    let notes = response.json::<Vec<NoteResponse>>().await?;
    Ok(notes)
}

pub async fn create_note(base_url: &str, note: CreateNoteRequest) -> Result<NoteResponse, Error> {
    let client = reqwest::Client::new();
    let url = format!("{}/{FLAT_API}", base_url);
    let response = client.post(url)
        .json(&note)
        .send()
        .await?
        .error_for_status()?;
    let created_note = response.json::<NoteResponse>().await?;
    Ok(created_note)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_notes() {
        let base_url = BASE_URL;
        let result = fetch_notes(base_url).await;
        assert!(result.is_ok());
        let notes = result.unwrap();
        assert!(!notes.is_empty());
    }

    #[tokio::test]
    async fn test_fetch_note() {
        let base_url = BASE_URL;
        // First get all notes to find a valid ID
        let notes = fetch_notes(base_url).await.unwrap();
        let first_note_id = notes[0].id;

        let result = fetch_note(base_url, first_note_id).await;
        assert!(result.is_ok());
        let note = result.unwrap();
        assert_eq!(note.id, first_note_id);
    }

    #[tokio::test]
    async fn test_create_note() {
        let base_url = BASE_URL;
        let note = CreateNoteRequest {
            title: "Test Note".to_string(),
            content: "This is a test note".to_string()
        };

        let result = create_note(base_url, note).await;
        assert!(result.is_ok());
        let created_note = result.unwrap();
        assert!(!created_note.title.is_empty());
        assert!(!created_note.content.is_empty());
    }
}
