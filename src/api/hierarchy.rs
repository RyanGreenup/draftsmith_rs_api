use axum::{extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use crate::schema::note_hierarchy::dsl::*;
use crate::tables::{NewNoteHierarchy, NoteHierarchy};
use crate::api::state::AppState;
use crate::api::AttachChildRequest;

pub fn is_circular_hierarchy(
    conn: &mut PgConnection,
    child_id: i32,
    potential_parent_id: Option<i32>,
) -> Result<bool, DieselError> {
    use crate::schema::note_hierarchy::dsl::*;
    let mut current_parent_id = potential_parent_id;
    while let Some(pid) = current_parent_id {
        if pid == child_id {
            return Ok(true); // Circular hierarchy detected
        }
        current_parent_id = note_hierarchy
            .filter(child_note_id.eq(pid))
            .select(parent_note_id)
            .first::<Option<i32>>(conn)
            .optional()?
            .flatten();
    }
    Ok(false)
}

pub async fn attach_child_note(
    State(state): State<AppState>,
    Json(payload): Json<AttachChildRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Prevent circular hierarchy
    if let Some(parent_id) = payload.parent_note_id {
        if is_circular_hierarchy(&mut conn, payload.child_note_id, Some(parent_id))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        {
            return Err(StatusCode::BAD_REQUEST); // Circular hierarchy detected
        }
    }

    // Check if a hierarchy entry already exists for the child
    let existing_entry = note_hierarchy
        .filter(child_note_id.eq(payload.child_note_id))
        .first::<NoteHierarchy>(&mut conn)
        .optional()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if existing_entry.is_some() {
        // Update the existing hierarchy entry
        diesel::update(note_hierarchy.filter(child_note_id.eq(payload.child_note_id)))
            .set((
                parent_note_id.eq(payload.parent_note_id),
                hierarchy_type.eq(payload.hierarchy_type.clone()),
            ))
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    } else {
        // Create a new hierarchy entry
        let new_entry = NewNoteHierarchy {
            child_note_id: Some(payload.child_note_id),
            parent_note_id: payload.parent_note_id,
            hierarchy_type: payload.hierarchy_type.as_deref(),
        };

        diesel::insert_into(note_hierarchy)
            .values(&new_entry)
            .execute(&mut conn)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }

    Ok(StatusCode::OK)
}
