use crate::schema::note_attributes;
use crate::schema::note_attributes::dsl::*;
use diesel::prelude::*;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::note_attributes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct NoteAttribute {
    pub id: i32,
    pub note_id: Option<i32>,
    pub attribute_id: Option<i32>,
    pub value: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::note_attributes)]
pub struct NewNoteAttribute<'a> {
    pub note_id: Option<i32>,
    pub attribute_id: Option<i32>,
    pub value: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attributes::{Attribute, NewAttribute};
    use crate::notes::{NewNote, Note};
    use crate::schema::attributes::dsl::*;
    use crate::schema::notes::dsl::*;
    use crate::test_utils::establish_test_connection;
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

    fn create_test_attribute(conn: &mut PgConnection) -> Attribute {
        let new_attribute = NewAttribute {
            name: "Test Attribute",
            description: Some("Test Description"),
        };

        diesel::insert_into(attributes)
            .values(&new_attribute)
            .get_result(conn)
            .expect("Error saving new attribute")
    }

    #[test]
    fn test_create_and_read_note_attribute() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            let test_note = create_test_note(conn);
            let test_attribute = create_test_attribute(conn);

            let new_note_attribute = NewNoteAttribute {
                note_id: Some(test_note.id),
                attribute_id: Some(test_attribute.id),
                value: "Test Value",
            };

            // Insert the note attribute
            let inserted_attr: NoteAttribute = diesel::insert_into(note_attributes)
                .values(&new_note_attribute)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_attr.note_id, Some(test_note.id));
            assert_eq!(inserted_attr.attribute_id, Some(test_attribute.id));
            assert_eq!(inserted_attr.value, "Test Value");

            // Read the note attribute back
            let found_attr = note_attributes
                .find(inserted_attr.id)
                .first::<NoteAttribute>(conn)?;

            // Verify the read data
            assert_eq!(found_attr.id, inserted_attr.id);
            assert_eq!(found_attr.note_id, Some(test_note.id));
            assert_eq!(found_attr.attribute_id, Some(test_attribute.id));
            assert_eq!(found_attr.value, "Test Value");

            Ok(())
        });
    }

    #[test]
    fn test_update_note_attribute() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            let test_note = create_test_note(conn);
            let test_attribute = create_test_attribute(conn);

            let new_note_attribute = NewNoteAttribute {
                note_id: Some(test_note.id),
                attribute_id: Some(test_attribute.id),
                value: "Initial Value",
            };

            // Insert the note attribute
            let inserted_attr: NoteAttribute = diesel::insert_into(note_attributes)
                .values(&new_note_attribute)
                .get_result(conn)?;

            // Update the note attribute
            let updated_attr = diesel::update(note_attributes.find(inserted_attr.id))
                .set(value.eq("Updated Value"))
                .get_result::<NoteAttribute>(conn)?;

            // Verify the update
            assert_eq!(updated_attr.value, "Updated Value");
            assert_eq!(updated_attr.note_id, Some(test_note.id));
            assert_eq!(updated_attr.attribute_id, Some(test_attribute.id));

            Ok(())
        });
    }
}
