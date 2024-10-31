use crate::schema::note_type_mappings;
use diesel::prelude::*;

#[derive(Debug, Queryable, Identifiable, Associations)]
#[diesel(belongs_to(Note, foreign_key = note_id))]
#[diesel(belongs_to(NoteType, foreign_key = type_id))]
#[diesel(primary_key(note_id, type_id))]
#[diesel(table_name = note_type_mappings)]
pub struct NoteTypeMapping {
    pub note_id: i32,
    pub type_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = note_type_mappings)]
pub struct NewNoteTypeMapping {
    pub note_id: i32,
    pub type_id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{NewNote, Note};
    use crate::note_types::{NewNoteType, NoteType};
    use crate::schema::note_type_mappings::dsl::*;
    use crate::schema::note_types::dsl::note_types;
    use crate::schema::notes::dsl::notes;
    use crate::test_utils::establish_test_connection;
    use diesel::prelude::*;

    fn create_test_note<'a>(conn: &mut PgConnection, title: &'a str) -> Note {
        let new_note = NewNote {
            title,
            content: "Test Content",
        };

        diesel::insert_into(notes)
            .values(&new_note)
            .get_result(conn)
            .expect("Error saving new note")
    }

    fn create_test_note_type<'a>(conn: &mut PgConnection, name: &'a str) -> NoteType {
        let new_note_type = NewNoteType {
            name,
            description: Some("Test Note Type"),
        };

        diesel::insert_into(note_types)
            .values(&new_note_type)
            .get_result(conn)
            .expect("Error saving new note type")
    }

    #[test]
    fn test_create_note_type_mapping() {
        let mut conn = establish_test_connection();
        conn.test_transaction::<_, diesel::result::Error, _>(|txn_conn| {
            // Create test Note and NoteType
            let test_note = create_test_note(txn_conn, "Test Note for Mapping");
            let test_note_type = create_test_note_type(txn_conn, "Test Note Type");

            // Create a new NoteTypeMapping
            let new_mapping = NewNoteTypeMapping {
                note_id: test_note.id,
                type_id: test_note_type.id,
            };

            diesel::insert_into(note_type_mappings)
                .values(&new_mapping)
                .execute(txn_conn)?;

            // Verify the mapping was created
            let mapping = note_type_mappings
                .find((test_note.id, test_note_type.id))
                .first::<NoteTypeMapping>(txn_conn)
                .expect("Error loading note_type_mapping");

            assert_eq!(mapping.note_id, test_note.id);
            assert_eq!(mapping.type_id, test_note_type.id);

            Ok(())
        });
    }

    #[test]
    fn test_delete_note_type_mapping() {
        let mut conn = establish_test_connection();
        conn.test_transaction::<_, diesel::result::Error, _>(|txn_conn| {
            // Create test Note and NoteType
            let test_note = create_test_note(txn_conn, "Test Note for Deletion");
            let test_note_type = create_test_note_type(txn_conn, "Test Note Type");

            // Create a new NoteTypeMapping
            let new_mapping = NewNoteTypeMapping {
                note_id: test_note.id,
                type_id: test_note_type.id,
            };

            diesel::insert_into(note_type_mappings)
                .values(&new_mapping)
                .execute(txn_conn)?;

            // Delete the NoteTypeMapping
            let num_deleted = diesel::delete(
                note_type_mappings.filter(
                    note_id
                        .eq(test_note.id)
                        .and(type_id.eq(test_note_type.id)),
                ),
            )
            .execute(txn_conn)?;

            assert_eq!(num_deleted, 1);

            // Verify the mapping was deleted
            let mapping = note_type_mappings
                .find((test_note.id, test_note_type.id))
                .first::<NoteTypeMapping>(txn_conn)
                .optional()?;

            assert!(mapping.is_none());

            Ok(())
        });
    }
}
