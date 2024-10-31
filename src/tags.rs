use diesel::prelude::*;
use crate::schema::tags;
use crate::schema::tags::dsl::*;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = tags)]
pub struct Tag {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable)]
#[diesel(table_name = tags)]
pub struct NewTag<'a> {
    pub name: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::establish_test_connection;
    use diesel::connection::Connection;

    #[test]
    fn test_create_and_read_tag() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a new tag
            let new_tag = NewTag {
                name: "Test Tag",
            };

            // Insert the tag
            let inserted_tag: Tag = diesel::insert_into(tags)
                .values(&new_tag)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_tag.name, "Test Tag");

            // Read the tag back
            let found_tag = tags.find(inserted_tag.id).first::<Tag>(conn)?;

            // Verify the read data
            assert_eq!(found_tag.id, inserted_tag.id);
            assert_eq!(found_tag.name, "Test Tag");

            Ok(())
        });
    }

    #[test]
    fn test_update_tag() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create initial tag
            let new_tag = NewTag {
                name: "Initial Name",
            };

            // Insert the tag
            let inserted_tag: Tag = diesel::insert_into(tags)
                .values(&new_tag)
                .get_result(conn)?;

            // Update the tag
            let updated_tag = diesel::update(tags.find(inserted_tag.id))
                .set(name.eq("Updated Name"))
                .get_result::<Tag>(conn)?;

            // Verify the update
            assert_eq!(updated_tag.name, "Updated Name");

            Ok(())
        });
    }

    #[test]
    fn test_delete_tag() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a tag
            let new_tag = NewTag {
                name: "Tag to Delete",
            };

            // Insert the tag
            let inserted_tag: Tag = diesel::insert_into(tags)
                .values(&new_tag)
                .get_result(conn)?;

            // Delete the tag
            let deleted_count = diesel::delete(tags.find(inserted_tag.id))
                .execute(conn)?;

            // Verify one record was deleted
            assert_eq!(deleted_count, 1);

            // Verify the tag no longer exists
            let find_result = tags.find(inserted_tag.id).first::<Tag>(conn);
            assert!(find_result.is_err());

            Ok(())
        });
    }
}
