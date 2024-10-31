use diesel::prelude::*;
use crate::schema::note_tags;
use crate::schema::note_tags::dsl::*;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = note_tags)]
#[diesel(primary_key(note_id, tag_id))]
pub struct NoteTag {
    pub note_id: i32,
    pub tag_id: i32,
}

#[derive(Insertable)]
#[diesel(table_name = note_tags)]
pub struct NewNoteTag {
    pub note_id: i32,
    pub tag_id: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::establish_test_connection;
    use crate::notes::{Note, NewNote};
    use crate::tags::{Tag, NewTag};
    use crate::schema::notes::dsl::*;
    use crate::schema::tags::dsl::*;
    use diesel::connection::Connection;

    fn create_test_note(conn: &mut PgConnection) -> Note {
        let new_note = NewNote {
            title: "Test Note",
            content: "Test Content",
        };

        diesel::insert_into(notes)
            .values(&new_note)
            .get_result(conn)
            .expect("Error saving new note")
    }

    fn create_test_tag(conn: &mut PgConnection) -> Tag {
        let new_tag = NewTag {
            name: "Test Tag",
        };

        diesel::insert_into(tags)
            .values(&new_tag)
            .get_result(conn)
            .expect("Error saving new tag")
    }

    #[test]
    fn test_create_and_read_note_tag() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a note and tag first
            let test_note = create_test_note(conn);
            let test_tag = create_test_tag(conn);

            // Create a new note_tag
            let new_note_tag = NewNoteTag {
                note_id: test_note.id,
                tag_id: test_tag.id,
            };

            // Insert the note_tag
            let inserted_note_tag: NoteTag = diesel::insert_into(note_tags)
                .values(&new_note_tag)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_note_tag.note_id, test_note.id);
            assert_eq!(inserted_note_tag.tag_id, test_tag.id);

            // Read the note_tag back
            let found_note_tag = note_tags
                .filter(note_id.eq(test_note.id))
                .filter(tag_id.eq(test_tag.id))
                .first::<NoteTag>(conn)?;

            // Verify the read data
            assert_eq!(found_note_tag.note_id, test_note.id);
            assert_eq!(found_note_tag.tag_id, test_tag.id);

            Ok(())
        });
    }

    #[test]
    fn test_delete_note_tag() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a note and tag first
            let test_note = create_test_note(conn);
            let test_tag = create_test_tag(conn);

            // Create a note_tag
            let new_note_tag = NewNoteTag {
                note_id: test_note.id,
                tag_id: test_tag.id,
            };

            // Insert the note_tag
            let inserted_note_tag: NoteTag = diesel::insert_into(note_tags)
                .values(&new_note_tag)
                .get_result(conn)?;

            // Delete the note_tag
            let deleted_count = diesel::delete(
                note_tags
                    .filter(note_id.eq(inserted_note_tag.note_id))
                    .filter(tag_id.eq(inserted_note_tag.tag_id))
            ).execute(conn)?;

            // Verify one record was deleted
            assert_eq!(deleted_count, 1);

            // Verify the note_tag no longer exists
            let find_result = note_tags
                .filter(note_id.eq(inserted_note_tag.note_id))
                .filter(tag_id.eq(inserted_note_tag.tag_id))
                .first::<NoteTag>(conn);
            assert!(find_result.is_err());

            Ok(())
        });
    }
}
