use reqwest::Error;
use crate::BASE_URL;
use crate::api::NoteResponse;

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
}
