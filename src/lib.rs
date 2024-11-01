pub mod schema;

use diesel::deserialize::{FromSql, Result};
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::{AsExpression, FromSqlRow};
use schema::*;
use std::io::Write;

#[derive(Debug, Clone, AsExpression, FromSqlRow)]
#[diesel(sql_type = crate::schema::sql_types::Tsvector)]
pub struct Tsvector(pub String);

impl ToSql<crate::schema::sql_types::Tsvector, Pg> for Tsvector {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> diesel::serialize::Result {
        out.write_all(self.0.as_bytes())?;
        Ok(IsNull::No)
    }
}

impl FromSql<crate::schema::sql_types::Tsvector, Pg> for Tsvector {
    fn from_sql(bytes: PgValue) -> Result<Self> {
        let string = String::from_utf8(bytes.as_bytes().to_vec())?;
        Ok(Tsvector(string))
    }
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::assets)]
pub struct Asset {
    pub id: i32,
    pub note_id: Option<i32>,
    pub location: String,
    pub description: Option<String>,
    pub description_tsv: Option<Tsvector>,
    pub created_at: Option<chrono::NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::assets)]
pub struct NewAsset<'a> {
    pub note_id: Option<i32>,
    pub location: &'a str,
    pub description: Option<&'a str>,
}

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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = journal_entries)]
pub struct JournalEntry {
    pub id: i32,
    pub note_id: Option<i32>,
    pub entry_date: chrono::NaiveDate,
}

#[derive(Insertable)]
#[diesel(table_name = journal_entries)]
pub struct NewJournalEntry {
    pub note_id: Option<i32>,
    pub entry_date: chrono::NaiveDate,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = note_attributes)]
pub struct NoteAttribute {
    pub id: i32,
    pub note_id: Option<i32>,
    pub attribute_id: Option<i32>,
    pub value: String,
}

#[derive(Insertable)]
#[diesel(table_name = note_attributes)]
pub struct NewNoteAttribute<'a> {
    pub note_id: Option<i32>,
    pub attribute_id: Option<i32>,
    pub value: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = note_hierarchy)]
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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = note_modifications)]
pub struct NoteModification {
    pub id: i32,
    pub note_id: Option<i32>,
    pub previous_content: String,
    pub modified_at: Option<chrono::NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = note_modifications)]
pub struct NewNoteModification<'a> {
    pub note_id: Option<i32>,
    pub previous_content: &'a str,
    pub modified_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = note_tags)]
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

#[derive(Debug, Queryable, Selectable)]
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

#[derive(Debug, Queryable, Selectable)]
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

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = notes)]
pub struct Note {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub fts: Option<Tsvector>,
}

#[derive(Insertable)]
#[diesel(table_name = notes)]
pub struct NewNote<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = tag_hierarchy)]
pub struct TagHierarchy {
    pub id: i32,
    pub parent_tag_id: Option<i32>,
    pub child_tag_id: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = tag_hierarchy)]
pub struct NewTagHierarchy {
    pub parent_tag_id: Option<i32>,
    pub child_tag_id: Option<i32>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::tags)]
pub struct Tag {
    pub id: i32,
    pub name: String,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::tags)]
pub struct NewTag<'a> {
    pub name: &'a str,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = task_clocks)]
pub struct TaskClock {
    pub id: i32,
    pub task_id: Option<i32>,
    pub clock_in: chrono::NaiveDateTime,
    pub clock_out: Option<chrono::NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = task_clocks)]
pub struct NewTaskClock {
    pub task_id: Option<i32>,
    pub clock_in: chrono::NaiveDateTime,
    pub clock_out: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = task_schedules)]
pub struct TaskSchedule {
    pub id: i32,
    pub task_id: i32,
    pub start_datetime: Option<chrono::NaiveDateTime>,
    pub end_datetime: Option<chrono::NaiveDateTime>,
}

#[derive(Insertable)]
#[diesel(table_name = task_schedules)]
pub struct NewTaskSchedule {
    pub task_id: i32,
    pub start_datetime: Option<chrono::NaiveDateTime>,
    pub end_datetime: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = tasks)]
pub struct Task {
    pub id: i32,
    pub note_id: Option<i32>,
    pub status: String,
    pub effort_estimate: Option<bigdecimal::BigDecimal>,
    pub actual_effort: Option<bigdecimal::BigDecimal>,
    pub deadline: Option<chrono::NaiveDateTime>,
    pub priority: Option<i32>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = tasks)]
pub struct NewTask<'a> {
    pub note_id: Option<i32>,
    pub status: &'a str,
    pub effort_estimate: Option<bigdecimal::BigDecimal>,
    pub actual_effort: Option<bigdecimal::BigDecimal>,
    pub deadline: Option<chrono::NaiveDateTime>,
    pub priority: Option<i32>,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub all_day: Option<bool>,
    pub goal_relationship: Option<i32>,
}

#[cfg(test)]
mod tags {
    use super::utils::*;
    use super::*;
    use crate::schema::tags::table;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;

    pub struct TagTests;

    impl CrudTest for TagTests {
        type Model = Tag;

        fn test_create() {
            let mut conn = establish_test_connection();
            let tag = setup_test_tag(&mut conn);

            let found_tag = table
                .find(tag.id)
                .select(Tag::as_select())
                .get_result(&mut conn)
                .expect("Error loading tag");

            assert_eq!(found_tag.name, "Test Tag");
        }

        fn test_read() {
            let mut conn = establish_test_connection();
            let created_tag = setup_test_tag(&mut conn);

            let found_tag = table
                .find(created_tag.id)
                .select(Tag::as_select())
                .get_result(&mut conn)
                .expect("Error loading tag");

            assert_eq!(found_tag.id, created_tag.id);
            assert_eq!(found_tag.name, "Test Tag");
        }

        fn test_update() {
            let mut conn = establish_test_connection();
            let tag = setup_test_tag(&mut conn);

            let updated_rows = diesel::update(table.find(tag.id))
                .set(crate::schema::tags::dsl::name.eq("Updated Test Tag"))
                .execute(&mut conn)
                .expect("Error updating tag");

            assert_eq!(updated_rows, 1);

            let updated_tag = table
                .find(tag.id)
                .select(Tag::as_select())
                .get_result(&mut conn)
                .expect("Error loading updated tag");

            assert_eq!(updated_tag.name, "Updated Test Tag");
        }

        fn test_delete() {
            let mut conn = establish_test_connection();
            let tag = setup_test_tag(&mut conn);

            let deleted_rows = diesel::delete(table.find(tag.id))
                .execute(&mut conn)
                .expect("Error deleting tag");

            assert_eq!(deleted_rows, 1);

            let find_result = table
                .find(tag.id)
                .select(Tag::as_select())
                .get_result::<Tag>(&mut conn);

            assert!(find_result.is_err());
        }
    }

    #[test]
    fn test_crud_tag() {
        TagTests::test_create();
        TagTests::test_read();
        TagTests::test_update();
        TagTests::test_delete();
    }

    #[test]
    fn test_create_tag() {
        let mut conn = establish_test_connection();
        let tag = setup_test_tag(&mut conn);

        let found_tag = table
            .find(tag.id)
            .select(Tag::as_select())
            .get_result(&mut conn)
            .expect("Error loading tag");

        assert_eq!(found_tag.name, "Test Tag");
    }

    #[test]
    fn test_read_tag() {
        let mut conn = establish_test_connection();
        let created_tag = setup_test_tag(&mut conn);

        let found_tag = table
            .find(created_tag.id)
            .select(Tag::as_select())
            .get_result(&mut conn)
            .expect("Error loading tag");

        assert_eq!(found_tag.id, created_tag.id);
        assert_eq!(found_tag.name, "Test Tag");
    }

    #[test]
    fn test_update_tag() {
        let mut conn = establish_test_connection();
        let tag = setup_test_tag(&mut conn);

        let updated_rows = diesel::update(table.find(tag.id))
            .set(crate::schema::tags::dsl::name.eq("Updated Test Tag"))
            .execute(&mut conn)
            .expect("Error updating tag");

        assert_eq!(updated_rows, 1);

        let updated_tag = table
            .find(tag.id)
            .select(Tag::as_select())
            .get_result(&mut conn)
            .expect("Error loading updated tag");

        assert_eq!(updated_tag.name, "Updated Test Tag");
    }

    #[test]
    fn test_delete_tag() {
        let mut conn = establish_test_connection();
        let tag = setup_test_tag(&mut conn);

        let deleted_rows = diesel::delete(table.find(tag.id))
            .execute(&mut conn)
            .expect("Error deleting tag");

        assert_eq!(deleted_rows, 1);

        let find_result = table
            .find(tag.id)
            .select(Tag::as_select())
            .get_result::<Tag>(&mut conn);

        assert!(find_result.is_err());
    }
}

/// Trait for defining a set of tests that ensure all CRUD operations are covered.
///
/// Implement this trait for each table to create a standardized test suite for creating, reading,
/// updating, and deleting records in the database.
pub trait CrudTest {
    /// The associated model type for which these CRUD tests will be performed.
    type Model;

    /// Test the creation of a new record.
    ///
    /// This function should insert a new record into the database and verify that it was created
    /// successfully. It should check that the returned record matches the expected values.
    fn test_create();

    /// Test the retrieval of an existing record.
    ///
    /// This function should retrieve a previously inserted record from the database and verify
    /// that its contents match the expected values.
    fn test_read();

    /// Test the updating of an existing record.
    ///
    /// This function should update a previously inserted record in the database and verify that
    /// the changes were applied correctly. It should check that the updated record matches the new
    /// expected values.
    fn test_update();

    /// Test the deletion of an existing record.
    ///
    /// This function should delete a previously inserted record from the database and verify that
    /// it can no longer be retrieved. It should ensure that attempting to read the deleted record
    /// results in an error or `None`.
    fn test_delete();
}

#[cfg(test)]
mod utils {
    use super::*;
    use diesel::pg::PgConnection;
    use dotenv::dotenv;
    use std::env;

    pub fn establish_test_connection() -> PgConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&database_url)
            .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
    }

    pub fn setup_test_note(conn: &mut PgConnection) -> Note {
        // Create a test note
        let new_note = NewNote {
            title: "Test Note",
            content: "This is a test note content",
            created_at: Some(chrono::Local::now().naive_local()),
            modified_at: Some(chrono::Local::now().naive_local()),
        };

        diesel::insert_into(notes::table)
            .values(&new_note)
            .get_result(conn)
            .expect("Error saving new note")
    }

    pub fn setup_test_tag(conn: &mut PgConnection) -> Tag {
        use crate::schema::tags::dsl::*;
        let new_tag = NewTag { name: "Test Tag" };

        diesel::insert_into(tags)
            .values(&new_tag)
            .get_result(conn)
            .expect("Error saving new tag")
    }

    pub fn setup_test_asset(conn: &mut PgConnection) -> Asset {
        let note = setup_test_note(conn);
        let new_asset = NewAsset {
            note_id: Some(note.id),
            location: "/test/path/file.txt",
            description: Some("Test asset description"),
        };

        diesel::insert_into(assets::table)
            .values(&new_asset)
            .get_result(conn)
            .expect("Error saving new asset")
    }
}

#[cfg(test)]
mod assets {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;

    pub struct AssetTests;

    impl CrudTest for AssetTests {
        type Model = Asset;

        fn test_create() {
            let mut conn = establish_test_connection();
            let asset = setup_test_asset(&mut conn);

            let found_asset = assets::table
                .find(asset.id)
                .select(Asset::as_select())
                .first(&mut conn)
                .expect("Error loading asset");

            assert_eq!(found_asset.location, "/test/path/file.txt");
            assert_eq!(found_asset.description, Some("Test asset description".to_string()));
        }

        fn test_read() {
            let mut conn = establish_test_connection();
            let created_asset = setup_test_asset(&mut conn);

            let found_asset = assets::table
                .find(created_asset.id)
                .select(Asset::as_select())
                .first(&mut conn)
                .expect("Error loading asset");

            assert_eq!(found_asset.id, created_asset.id);
            assert_eq!(found_asset.location, "/test/path/file.txt");
            assert_eq!(found_asset.description, Some("Test asset description".to_string()));
        }

        fn test_update() {
            let mut conn = establish_test_connection();
            let asset = setup_test_asset(&mut conn);

            let updated_rows = diesel::update(table.find(asset.id))
                .set((
                    location.eq("/updated/path/file.txt"),
                    description.eq(Some("Updated description")),
                ))
                .execute(&mut conn)
                .expect("Error updating asset");

            assert_eq!(updated_rows, 1);

            let updated_asset = assets::table
                .find(asset.id)
                .select(Asset::as_select())
                .first(&mut conn)
                .expect("Error loading updated asset");

            assert_eq!(updated_asset.location, "/updated/path/file.txt");
            assert_eq!(updated_asset.description, Some("Updated description".to_string()));
        }

        fn test_delete() {
            let mut conn = establish_test_connection();
            let asset = setup_test_asset(&mut conn);

            let deleted_rows = diesel::delete(assets::table.find(asset.id))
                .execute(&mut conn)
                .expect("Error deleting asset");

            assert_eq!(deleted_rows, 1);

            let find_result = assets::table
                .find(asset.id)
                .select(Asset::as_select())
                .first::<Asset>(&mut conn);

            assert!(find_result.is_err());
        }
    }

    #[test]
    fn test_crud_asset() {
        AssetTests::test_create();
        AssetTests::test_read();
        AssetTests::test_update();
        AssetTests::test_delete();
    }
}

#[cfg(test)]
mod tests {
    use super::utils::*;
    use super::*;
    use crate::schema::notes;

    pub struct NoteTests;

    impl CrudTest for NoteTests {
        type Model = Note;

        fn test_create() {
            let mut conn = establish_test_connection();

            let note = setup_test_note(&mut conn);

            assert_eq!(note.title, "Test Note");
            assert_eq!(note.content, "This is a test note content");
            assert!(note.created_at.is_some());
            assert!(note.modified_at.is_some());
        }

        fn test_read() {
            let mut conn = establish_test_connection();
            let created_note = setup_test_note(&mut conn);

            let found_note = notes::table
                .find(created_note.id)
                .select(Note::as_select())
                .first(&mut conn)
                .expect("Error loading note");

            assert_eq!(found_note.id, created_note.id);
            assert_eq!(found_note.title, "Test Note");
        }

        fn test_update() {
            let mut conn = establish_test_connection();
            let note = setup_test_note(&mut conn);

            let updated_rows = diesel::update(notes::table.find(note.id))
                .set(notes::title.eq("Updated Test Note"))
                .execute(&mut conn)
                .expect("Error updating note");

            assert_eq!(updated_rows, 1);

            let updated_note = notes::table
                .find(note.id)
                .select(Note::as_select())
                .first(&mut conn)
                .expect("Error loading updated note");

            assert_eq!(updated_note.title, "Updated Test Note");
        }

        fn test_delete() {
            let mut conn = establish_test_connection();
            let note = setup_test_note(&mut conn);

            let deleted_rows = diesel::delete(notes::table.find(note.id))
                .execute(&mut conn)
                .expect("Error deleting note");

            assert_eq!(deleted_rows, 1);

            let find_result = notes::table
                .find(note.id)
                .select(Note::as_select())
                .first::<Note>(&mut conn);

            assert!(find_result.is_err());
        }
    }

    #[test]
    fn test_crud_note() {
        NoteTests::test_create();
        NoteTests::test_read();
        NoteTests::test_update();
        NoteTests::test_delete();
    }

    #[test]
    fn test_create_note() {
        let mut conn = establish_test_connection();

        let note = setup_test_note(&mut conn);

        assert_eq!(note.title, "Test Note");
        assert_eq!(note.content, "This is a test note content");
        assert!(note.created_at.is_some());
        assert!(note.modified_at.is_some());
    }

    #[test]
    fn test_read_note() {
        let mut conn = establish_test_connection();
        let created_note = setup_test_note(&mut conn);

        let found_note = notes::table
            .find(created_note.id)
            .select(Note::as_select())
            .first(&mut conn)
            .expect("Error loading note");

        assert_eq!(found_note.id, created_note.id);
        assert_eq!(found_note.title, "Test Note");
    }

    #[test]
    fn test_update_note() {
        let mut conn = establish_test_connection();
        let note = setup_test_note(&mut conn);

        let updated_rows = diesel::update(notes::table.find(note.id))
            .set(notes::title.eq("Updated Test Note"))
            .execute(&mut conn)
            .expect("Error updating note");

        assert_eq!(updated_rows, 1);

        let updated_note = notes::table
            .find(note.id)
            .select(Note::as_select())
            .first(&mut conn)
            .expect("Error loading updated note");

        assert_eq!(updated_note.title, "Updated Test Note");
    }

    #[test]
    fn test_delete_note() {
        let mut conn = establish_test_connection();
        let note = setup_test_note(&mut conn);

        let deleted_rows = diesel::delete(notes::table.find(note.id))
            .execute(&mut conn)
            .expect("Error deleting note");

        assert_eq!(deleted_rows, 1);

        let find_result = notes::table
            .find(note.id)
            .select(Note::as_select())
            .first::<Note>(&mut conn);

        assert!(find_result.is_err());
    }
}
