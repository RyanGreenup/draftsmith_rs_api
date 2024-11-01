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
#[diesel(table_name = crate::schema::attributes)]
pub struct Attribute {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::attributes)]
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
#[diesel(table_name = crate::schema::notes)]
pub struct Note {
    pub id: i32,
    pub title: String,
    pub content: String,
    pub created_at: Option<chrono::NaiveDateTime>,
    pub modified_at: Option<chrono::NaiveDateTime>,
    pub fts: Option<Tsvector>,
}

#[derive(Insertable)]
#[diesel(table_name = crate::schema::notes)]
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
#[diesel(table_name = crate::schema::tasks)]
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
#[diesel(table_name = crate::schema::tasks)]
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
mod tests {
    use super::*;
    use diesel::result::Error as DieselError;

    fn establish_connection() -> PgConnection {
        dotenv::dotenv().ok();
        let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in .env file");
        PgConnection::establish(&database_url)
            .expect("Error connecting to database")
    }

    #[test]
    fn test_note_crud() {
        let conn = &mut establish_connection();
        
        conn.test_transaction(|conn| {
            // Test Create
            let new_note = NewNote {
                title: "Test Note",
                content: "This is a test note",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_note = diesel::insert_into(notes::table)
                .values(&new_note)
                .get_result::<Note>(conn)
                .expect("Error saving new note");

            dbg!(format!("Created Note #: {:?}", created_note.id));
            assert_eq!(created_note.title, "Test Note");
            assert_eq!(created_note.content, "This is a test note");

            // Test Read
            let read_note = notes::table
                .find(created_note.id)
                .first::<Note>(conn)
                .expect("Error loading note");

            assert_eq!(read_note.id, created_note.id);
            assert_eq!(read_note.title, created_note.title);

            // Test Update
            let updated_note = diesel::update(notes::table.find(created_note.id))
                .set(notes::content.eq("Updated content"))
                .get_result::<Note>(conn)
                .expect("Error updating note");

            assert_eq!(updated_note.content, "Updated content");

            dbg!(format!("Deleting Note #: {:?}", created_note.id));
            // Test Delete
            let deleted_count = diesel::delete(notes::table.find(created_note.id))
                .execute(conn)
                .expect("Error deleting note");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = notes::table
                .find(created_note.id)
                .first::<Note>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_tag_crud() {
        let conn = &mut establish_connection();
        
        conn.test_transaction(|conn| {
            // Test Create
            let new_tag = NewTag {
                name: "Test Tag",
            };

            let created_tag = diesel::insert_into(tags::table)
                .values(&new_tag)
                .get_result::<Tag>(conn)
                .expect("Error saving new tag");

            dbg!(format!("Created Tag #: {:?}", created_tag.id));
            assert_eq!(created_tag.name, "Test Tag");

            // Test Read
            let read_tag = tags::table
                .find(created_tag.id)
                .first::<Tag>(conn)
                .expect("Error loading tag");

            assert_eq!(read_tag.id, created_tag.id);
            assert_eq!(read_tag.name, created_tag.name);

            // Test Update
            let updated_tag = diesel::update(tags::table.find(created_tag.id))
                .set(tags::name.eq("Updated Tag"))
                .get_result::<Tag>(conn)
                .expect("Error updating tag");

            assert_eq!(updated_tag.name, "Updated Tag");

            dbg!(format!("Deleting Tag #: {:?}", created_tag.id));
            // Test Delete
            let deleted_count = diesel::delete(tags::table.find(created_tag.id))
                .execute(conn)
                .expect("Error deleting tag");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = tags::table
                .find(created_tag.id)
                .first::<Tag>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_assets_crud() {
        let conn = &mut establish_connection();
        
        conn.test_transaction(|conn| {
            // Test Create
            let new_asset = NewAsset {
                note_id: None,
                location: "/path/to/test/asset.jpg",
                description: Some("Test asset description"),
            };

            let created_asset = diesel::insert_into(assets::table)
                .values(&new_asset)
                .get_result::<Asset>(conn)
                .expect("Error saving new asset");

            dbg!(format!("Created Asset #: {:?}", created_asset.id));
            assert_eq!(created_asset.location, "/path/to/test/asset.jpg");
            assert_eq!(created_asset.description, Some("Test asset description".to_string()));

            // Test Read
            let read_asset = assets::table
                .find(created_asset.id)
                .first::<Asset>(conn)
                .expect("Error loading asset");

            assert_eq!(read_asset.id, created_asset.id);
            assert_eq!(read_asset.location, created_asset.location);

            // Test Update
            let updated_asset = diesel::update(assets::table.find(created_asset.id))
                .set(assets::description.eq(Some("Updated description")))
                .get_result::<Asset>(conn)
                .expect("Error updating asset");

            assert_eq!(updated_asset.description, Some("Updated description".to_string()));

            dbg!(format!("Deleting Asset #: {:?}", created_asset.id));
            // Test Delete
            let deleted_count = diesel::delete(assets::table.find(created_asset.id))
                .execute(conn)
                .expect("Error deleting asset");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = assets::table
                .find(created_asset.id)
                .first::<Asset>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_note_tags_crud() {
        let conn = &mut establish_connection();
        
        conn.test_transaction(|conn| {
            // First create a note and tag to work with
            let new_note = NewNote {
                title: "Test Note for Tags",
                content: "This is a test note for tag testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_note = diesel::insert_into(notes::table)
                .values(&new_note)
                .get_result::<Note>(conn)
                .expect("Error saving new note");

            let new_tag = NewTag {
                name: "Test Tag for Note",
            };

            let created_tag = diesel::insert_into(tags::table)
                .values(&new_tag)
                .get_result::<Tag>(conn)
                .expect("Error saving new tag");

            // Test Create
            let new_note_tag = NewNoteTag {
                note_id: created_note.id,
                tag_id: created_tag.id,
            };

            let created_note_tag = diesel::insert_into(note_tags::table)
                .values(&new_note_tag)
                .get_result::<NoteTag>(conn)
                .expect("Error saving new note_tag");

            assert_eq!(created_note_tag.note_id, created_note.id);
            assert_eq!(created_note_tag.tag_id, created_tag.id);

            // Test Read
            let read_note_tag = note_tags::table
                .filter(note_tags::note_id.eq(created_note.id))
                .filter(note_tags::tag_id.eq(created_tag.id))
                .first::<NoteTag>(conn)
                .expect("Error loading note_tag");

            assert_eq!(read_note_tag.note_id, created_note_tag.note_id);
            assert_eq!(read_note_tag.tag_id, created_note_tag.tag_id);

            // Test Delete
            let deleted_count = diesel::delete(
                note_tags::table
                    .filter(note_tags::note_id.eq(created_note.id))
                    .filter(note_tags::tag_id.eq(created_tag.id))
            )
                .execute(conn)
                .expect("Error deleting note_tag");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = note_tags::table
                .filter(note_tags::note_id.eq(created_note.id))
                .filter(note_tags::tag_id.eq(created_tag.id))
                .first::<NoteTag>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }
}

