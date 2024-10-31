use diesel::prelude::*;
use crate::schema::assets;
use crate::schema::assets::dsl::*;
use crate::Tsvector;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = assets)]
pub struct Asset {
    pub id: i32,
    pub note_id: Option<i32>,
    pub location: String,
    pub description: Option<String>,
    pub description_tsv: Option<Tsvector>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = assets)]
pub struct NewAsset<'a> {
    pub note_id: Option<i32>,
    pub location: &'a str,
    pub description: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::Connection;
    use crate::test_utils::establish_test_connection;

    #[test]
    fn test_create_and_read_asset() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a new asset
            let new_asset = NewAsset {
                note_id: None,
                location: "/path/to/asset",
                description: Some("This is a test asset"),
            };

            // Insert the asset
            let inserted_asset: Asset = diesel::insert_into(assets)
                .values(&new_asset)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_asset.location, "/path/to/asset");
            assert_eq!(inserted_asset.description, Some("This is a test asset".to_string()));
            assert!(inserted_asset.note_id.is_none());
            assert!(inserted_asset.created_at.is_some());

            // Read the asset back
            let found_asset = assets.find(inserted_asset.id).first::<Asset>(conn)?;

            // Verify the read data
            assert_eq!(found_asset.id, inserted_asset.id);
            assert_eq!(found_asset.location, "/path/to/asset");
            assert_eq!(found_asset.description, Some("This is a test asset".to_string()));
            assert!(found_asset.note_id.is_none());

            Ok(())
        });
    }

    #[test]
    fn test_update_asset() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create initial asset
            let new_asset = NewAsset {
                note_id: None,
                location: "/initial/path",
                description: Some("Initial description"),
            };

            // Insert the asset
            let inserted_asset: Asset = diesel::insert_into(assets)
                .values(&new_asset)
                .get_result(conn)?;

            // Add a small delay to ensure timestamp difference
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Update the asset
            let updated_asset = diesel::update(assets.find(inserted_asset.id))
                .set((
                    description.eq(Some("Updated description")),
                    location.eq("/updated/path"),
                ))
                .get_result::<Asset>(conn)?;

            // Verify the update
            assert_eq!(updated_asset.description, Some("Updated description".to_string()));
            assert_eq!(updated_asset.location, "/updated/path");

            Ok(())
        });
    }
}
