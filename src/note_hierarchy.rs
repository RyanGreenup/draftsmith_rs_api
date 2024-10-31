use diesel::prelude::*;
use diesel::pg::PgConnection;
use crate::schema::note_hierarchy;

#[derive(Debug, Queryable)]
pub struct NoteHierarchy {
    pub id: i32,
    pub parent_note_id: Option<i32>,
    pub child_note_id: Option<i32>,
    pub hierarchy_type: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = note_hierarchy)]
pub struct NewNoteHierarchy<'a> {
    pub parent_note_id: Option<i32>,
    pub child_note_id: Option<i32>,
    pub hierarchy_type: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::Connection;
    use crate::test_utils::establish_test_connection;
    use crate::notes::{NewNote, Note};
    use crate::schema::notes::dsl::*;
    use diesel::RunQueryDsl;

    fn create_test_note<'a>(conn: &mut PgConnection, note_title: &'a str) -> Note {
        let new_note = NewNote {
            title: note_title,
            content: "Test Content",
        };

        diesel::insert_into(notes)
            .values(&new_note)
            .get_result(conn)
            .expect("Error saving new note")
    }

    #[test]
    fn test_create_note_hierarchy() {
        let mut conn = establish_test_connection();
        conn.test_transaction::<_, diesel::result::Error, _>(|| {
            // Create parent and child notes
            let parent_note = create_test_note(&mut conn, "Parent Note");
            let child_note = create_test_note(&mut conn, "Child Note");

            // Create a new note hierarchy entry
            let new_hierarchy = NewNoteHierarchy {
                parent_note_id: Some(parent_note.id),
                child_note_id: Some(child_note.id),
                hierarchy_type: Some("Parent-Child"),
            };

            let hierarchy: NoteHierarchy = diesel::insert_into(note_hierarchy::table)
                .values(&new_hierarchy)
                .get_result(&mut conn)?;

            // Assertions
            assert_eq!(hierarchy.parent_note_id, Some(parent_note.id));
            assert_eq!(hierarchy.child_note_id, Some(child_note.id));
            assert_eq!(hierarchy.hierarchy_type.as_deref(), Some("Parent-Child"));

            Ok(())
        });
    }
}
