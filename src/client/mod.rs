pub use crate::tables::{HierarchyMapping, NoteWithParent, NoteWithoutFts};
pub mod assets;
pub mod notes;
pub use crate::api::{
    compute_note_hash, AssetResponse, AttachChildRequest, BatchUpdateRequest, BatchUpdateResponse,
    CreateNoteRequest, ListAssetsParams, NoteHash, NoteTreeNode, UpdateAssetRequest,
    UpdateNoteRequest,
};
// Re-export the modules
pub use assets::*;
pub use notes::*;
