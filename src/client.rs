use reqwest::Error;
use crate::BASE_URL;
use crate::api::NoteResponse;

pub async fn fetch_note(base_url: &str, id: i32) -> Result<NoteResponse, Error> {
    let url = format!("{}/notes/{}", base_url, id);
    let response = reqwest::get(url).await?;
    let note = response.json::<NoteResponse>().await?;
    Ok(note)
}

pub async fn fetch_notes(base_url: &str) -> Result<Vec<NoteResponse>, Error> {
    let url = format!("{}/notes/flat", base_url);
    let response = reqwest::get(url).await?;
    let notes = response.json::<Vec<NoteResponse>>().await?;
    Ok(notes)
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
}
