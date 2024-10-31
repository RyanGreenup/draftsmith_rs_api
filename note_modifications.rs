use diesel::prelude::*;
use diesel::{Queryable, Insertable, Identifiable, Associations};
use chrono::NaiveDateTime;
use crate::schema::note_modifications;
use crate::notes::Note;

#[derive(Debug, Clone, Queryable, Identifiable, Associations)]
#[diesel(belongs_to(Note))]
#[diesel(table_name = note_modifications)]
pub struct NoteModification {
    pub id: i32,
    pub note_id: Option<i32>,
    pub previous_content: String,
    pub modified_at: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = note_modifications)]
pub struct NewNoteModification<'a> {
    pub note_id: Option<i32>,
    pub previous_content: &'a str,
    pub modified_at: Option<NaiveDateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::establish_test_connection;
    use crate::notes::{Note, NewNote};
    use crate::schema::notes::dsl::*;
    use crate::schema::note_modifications::dsl::*;
    use diesel::prelude::*;
    use diesel::result::Error;

    fn create_test_note<'a>(conn: &mut PgConnection) -> Note {
        let new_note = NewNote {
            title: "Test Note",
            content: "Original Content",
        };

        diesel::insert_into(notes)
            .values(&new_note)
            .get_result(conn)
            .expect("Error saving new note")
    }

    #[test]
    fn test_create_note_modification() {
        let mut conn = establish_test_connection();
        conn.test_transaction::<_, Error, _>(|txn_conn| {
            // Create a test note
            let note = create_test_note(txn_conn);

            // Create a new note modification
            let new_modification = NewNoteModification {
                note_id: Some(note.id),
                previous_content: &note.content,
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            // Insert the note modification
            let inserted_modification: NoteModification = diesel::insert_into(note_modifications)
                .values(&new_modification)
                .get_result(txn_conn)?;

            // Verify the insertion
            assert_eq!(inserted_modification.note_id, Some(note.id));
            assert_eq!(inserted_modification.previous_content, note.content);

            Ok(())
        });
    }

    #[test]
    fn test_delete_note_modification() {
        let mut conn = establish_test_connection();
        conn.test_transaction::<_, Error, _>(|txn_conn| {
            // Create a test note
            let note = create_test_note(txn_conn);

            // Insert a note modification
            let new_modification = NewNoteModification {
                note_id: Some(note.id),
                previous_content: &note.content,
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let inserted_modification: NoteModification = diesel::insert_into(note_modifications)
                .values(&new_modification)
                .get_result(txn_conn)?;

            // Delete the note modification
            let num_deleted = diesel::delete(
                note_modifications.filter(id.eq(inserted_modification.id))
            )
            .execute(txn_conn)?;

            // Verify deletion
            assert_eq!(num_deleted, 1);

            // Ensure the modification no longer exists
            let fetched_modification = note_modifications
                .filter(id.eq(inserted_modification.id))
                .first::<NoteModification>(txn_conn)
                .optional()?;

            assert!(fetched_modification.is_none());

            Ok(())
        });
    }
}
