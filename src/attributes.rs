use crate::schema::attributes;
use crate::schema::attributes::dsl::*;
use diesel::prelude::*;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = attributes)]
pub struct Attribute {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = attributes)]
pub struct NewAttribute<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::establish_test_connection;
    use diesel::connection::Connection;

    #[test]
    fn test_create_and_read_attribute() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a new attribute
            let new_attribute = NewAttribute {
                name: "Test Attribute",
                description: Some("Test Description"),
            };

            // Insert the attribute
            let inserted_attribute: Attribute = diesel::insert_into(attributes)
                .values(&new_attribute)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_attribute.name, "Test Attribute");
            assert_eq!(
                inserted_attribute.description,
                Some("Test Description".to_string())
            );

            // Read the attribute back
            let found_attribute = attributes
                .find(inserted_attribute.id)
                .first::<Attribute>(conn)?;

            // Verify the read data
            assert_eq!(found_attribute.id, inserted_attribute.id);
            assert_eq!(found_attribute.name, "Test Attribute");
            assert_eq!(
                found_attribute.description,
                Some("Test Description".to_string())
            );

            Ok(())
        });
    }

    #[test]
    fn test_update_attribute() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create initial attribute
            let new_attribute = NewAttribute {
                name: "Initial Name",
                description: Some("Initial description"),
            };

            // Insert the attribute
            let inserted_attribute: Attribute = diesel::insert_into(attributes)
                .values(&new_attribute)
                .get_result(conn)?;

            // Update the attribute
            let updated_attribute = diesel::update(attributes.find(inserted_attribute.id))
                .set((
                    name.eq("Updated Name"),
                    description.eq(Some("Updated description")),
                ))
                .get_result::<Attribute>(conn)?;

            // Verify the update
            assert_eq!(updated_attribute.name, "Updated Name");
            assert_eq!(
                updated_attribute.description,
                Some("Updated description".to_string())
            );

            Ok(())
        });
    }

    #[test]
    fn test_delete_attribute() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create an attribute
            let new_attribute = NewAttribute {
                name: "Attribute to Delete",
                description: Some("Will be deleted"),
            };

            // Insert the attribute
            let inserted_attribute: Attribute = diesel::insert_into(attributes)
                .values(&new_attribute)
                .get_result(conn)?;

            // Delete the attribute
            let deleted_count =
                diesel::delete(attributes.find(inserted_attribute.id)).execute(conn)?;

            // Verify one record was deleted
            assert_eq!(deleted_count, 1);

            // Verify the attribute no longer exists
            let find_result = attributes
                .find(inserted_attribute.id)
                .first::<Attribute>(conn);
            assert!(find_result.is_err());

            Ok(())
        });
    }

    #[test]
    fn test_create_attribute_without_description() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a new attribute without description
            let new_attribute = NewAttribute {
                name: "No Description Attribute",
                description: None,
            };

            // Insert the attribute
            let inserted_attribute: Attribute = diesel::insert_into(attributes)
                .values(&new_attribute)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_attribute.name, "No Description Attribute");
            assert_eq!(inserted_attribute.description, None);

            Ok(())
        });
    }
}
