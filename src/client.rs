use chrono::NaiveDateTime;
use reqwest::Error;
use serde::Deserialize;
use crate::lib;

#[derive(Debug, Deserialize)]
pub struct NoteResponse {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: Option<NaiveDateTime>,
    pub modified_at: Option<NaiveDateTime>,
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
        let base_url = lib::BASE_URL;
        let result = fetch_notes(base_url).await;
        assert!(result.is_ok());
        let notes = result.unwrap();
        assert!(!notes.is_empty());
    }
}
