use diesel::prelude::*;
use crate::schema::assets;
use crate::schema::assets::dsl::*;
use crate::Tsvector;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::assets)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct Asset {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub location: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub fts: Option<Tsvector>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::assets)]
pub struct NewAsset<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub location: &'a str,
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
                name: "Test Asset",
                description: "This is a test asset",
                location: "/path/to/asset",
            };

            // Insert the asset
            let inserted_asset: Asset = diesel::insert_into(assets)
                .values(&new_asset)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_asset.name, "Test Asset");
            assert_eq!(inserted_asset.description, "This is a test asset");
            assert_eq!(inserted_asset.location, "/path/to/asset");
            assert!(inserted_asset.created_at.is_some());
            assert!(inserted_asset.modified_at.is_some());

            // Read the asset back
            let found_asset = assets.find(inserted_asset.id).first::<Asset>(conn)?;

            // Verify the read data
            assert_eq!(found_asset.id, inserted_asset.id);
            assert_eq!(found_asset.name, "Test Asset");
            assert_eq!(found_asset.description, "This is a test asset");
            assert_eq!(found_asset.location, "/path/to/asset");

            Ok(())
        });
    }

    #[test]
    fn test_update_asset() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create initial asset
            let new_asset = NewAsset {
                name: "Initial Name",
                description: "Initial description",
                location: "/initial/path",
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
                    name.eq("Updated Name"),
                    description.eq("Updated description"),
                    location.eq("/updated/path"),
                ))
                .get_result::<Asset>(conn)?;

            // Verify the update
            assert_eq!(updated_asset.name, "Updated Name");
            assert_eq!(updated_asset.description, "Updated description");
            assert_eq!(updated_asset.location, "/updated/path");
            // TODO: Investigate timestamp assertion like in notes
            // assert!(updated_asset.modified_at.unwrap() > inserted_asset.modified_at.unwrap());

            Ok(())
        });
    }
}
