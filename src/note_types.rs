use crate::schema::note_types;
use diesel::prelude::*;

#[derive(Debug, Queryable, Identifiable)]
#[diesel(table_name = note_types)]
pub struct NoteType {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = note_types)]
pub struct NewNoteType<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::establish_test_connection;
    use diesel::prelude::*;
    use diesel::result::Error;

    fn create_test_note_type<'a>(conn: &mut PgConnection, name: &'a str) -> NoteType {
        let new_note_type = NewNoteType {
            name,
            description: Some("Test Note Type"),
        };

        diesel::insert_into(note_types::table)
            .values(&new_note_type)
            .get_result(conn)
            .expect("Error saving new note type")
    }

    #[test]
    fn test_create_and_read_note_type() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, Error, _>(|conn| {
            // Create a new note type
            let note_type = create_test_note_type(conn, "Test Type");

            // Verify the inserted data
            assert_eq!(note_type.name, "Test Type");
            assert_eq!(note_type.description.as_deref(), Some("Test Note Type"));

            // Read the note type back from the database
            let fetched_note_type = note_types::table
                .find(note_type.id)
                .get_result::<NoteType>(conn)?;

            // Verify the fetched data
            assert_eq!(fetched_note_type.id, note_type.id);
            assert_eq!(fetched_note_type.name, "Test Type");
            assert_eq!(fetched_note_type.description.as_deref(), Some("Test Note Type"));

            Ok(())
        });
    }

    #[test]
    fn test_update_note_type() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, Error, _>(|conn| {
            // Create a new note type
            let mut note_type = create_test_note_type(conn, "Original Type");

            // Update the note type
            note_type = diesel::update(note_types::table.find(note_type.id))
                .set((
                    note_types::name.eq("Updated Type"),
                    note_types::description.eq(Some("Updated Description")),
                ))
                .get_result::<NoteType>(conn)?;

            // Verify the updated data
            assert_eq!(note_type.name, "Updated Type");
            assert_eq!(note_type.description.as_deref(), Some("Updated Description"));

            Ok(())
        });
    }

    #[test]
    fn test_delete_note_type() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, Error, _>(|conn| {
            // Create a new note type
            let note_type = create_test_note_type(conn, "Type to Delete");

            // Delete the note type
            let deleted_rows = diesel::delete(note_types::table.find(note_type.id))
                .execute(conn)?;

            // Verify that one row was deleted
            assert_eq!(deleted_rows, 1);

            // Attempt to find the deleted note type
            let result = note_types::table.find(note_type.id).get_result::<NoteType>(conn);

            // Verify that the note type no longer exists
            assert!(matches!(result, Err(diesel::result::Error::NotFound)));

            Ok(())
        });
    }
}
