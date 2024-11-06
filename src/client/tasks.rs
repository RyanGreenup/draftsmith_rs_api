use reqwest::{self, StatusCode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Task not found")]
    NotFound,

    #[error("Unexpected server error: {0}")]
    ServerError(String),
}

// * Types ....................................................................

#[derive(Serialize)]
pub struct CreateTaskRequest {
    // TODO
}

#[derive(Deserialize)]
pub struct TagResponse {
    // TODO
}

// * Types ....................................................................
// * Client ...................................................................
// ** Flat Functions ..........................................................
// *** Create .................................................................
// *** Read ...................................................................
// **** Get Tag ...............................................................
// **** List Tags .............................................................
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
    // *** Functions ..........................................................
    // **** Create ............................................................
    // **** Read ..............................................................
    // **** Update ............................................................
    // **** Delete ............................................................
    // **** Tree ..............................................................
    // *** Utils ..............................................................
}