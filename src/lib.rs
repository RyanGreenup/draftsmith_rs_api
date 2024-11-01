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
    fn test_attributes_crud() {
        let conn = &mut establish_connection();

        conn.test_transaction(|conn| {
            // Test Create
            let new_attribute = NewAttribute {
                name: "Test Attribute",
                description: Some("Test attribute description"),
            };

            let created_attribute = diesel::insert_into(attributes::table)
                .values(&new_attribute)
                .get_result::<Attribute>(conn)
                .expect("Error saving new attribute");

            dbg!(format!("Created Attribute #: {:?}", created_attribute.id));
            assert_eq!(created_attribute.name, "Test Attribute");
            assert_eq!(created_attribute.description, Some("Test attribute description".to_string()));

            // Test Read
            let read_attribute = attributes::table
                .find(created_attribute.id)
                .first::<Attribute>(conn)
                .expect("Error loading attribute");

            assert_eq!(read_attribute.id, created_attribute.id);
            assert_eq!(read_attribute.name, created_attribute.name);

            // Test Update
            let updated_attribute = diesel::update(attributes::table.find(created_attribute.id))
                .set(attributes::description.eq(Some("Updated description")))
                .get_result::<Attribute>(conn)
                .expect("Error updating attribute");

            assert_eq!(updated_attribute.description, Some("Updated description".to_string()));

            dbg!(format!("Deleting Attribute #: {:?}", created_attribute.id));
            // Test Delete
            let deleted_count = diesel::delete(attributes::table.find(created_attribute.id))
                .execute(conn)
                .expect("Error deleting attribute");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = attributes::table
                .find(created_attribute.id)
                .first::<Attribute>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_journal_entries_crud() {
        let conn = &mut establish_connection();

        conn.test_transaction(|conn| {
            // First create a note to work with
            let new_note = NewNote {
                title: "Test Note for Journal Entry",
                content: "This is a test note for journal entry testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_note = diesel::insert_into(notes::table)
                .values(&new_note)
                .get_result::<Note>(conn)
                .expect("Error saving new note");

            // Test Create
            let new_journal_entry = NewJournalEntry {
                note_id: Some(created_note.id),
                entry_date: chrono::Local::now().naive_local().date(),
            };

            let created_entry = diesel::insert_into(journal_entries::table)
                .values(&new_journal_entry)
                .get_result::<JournalEntry>(conn)
                .expect("Error saving new journal entry");

            dbg!(format!("Created Journal Entry #: {:?}", created_entry.id));
            assert_eq!(created_entry.note_id, Some(created_note.id));
            assert_eq!(created_entry.entry_date, new_journal_entry.entry_date);

            // Test Read
            let read_entry = journal_entries::table
                .find(created_entry.id)
                .first::<JournalEntry>(conn)
                .expect("Error loading journal entry");

            assert_eq!(read_entry.id, created_entry.id);
            assert_eq!(read_entry.note_id, created_entry.note_id);
            assert_eq!(read_entry.entry_date, created_entry.entry_date);

            // Test Update
            let tomorrow = chrono::Local::now().naive_local().date().succ_opt().expect("Error getting tomorrow's date");
            let updated_entry = diesel::update(journal_entries::table.find(created_entry.id))
                .set(journal_entries::entry_date.eq(tomorrow))
                .get_result::<JournalEntry>(conn)
                .expect("Error updating journal entry");

            assert_eq!(updated_entry.entry_date, tomorrow);

            dbg!(format!("Deleting Journal Entry #: {:?}", created_entry.id));
            // Test Delete
            let deleted_count = diesel::delete(journal_entries::table.find(created_entry.id))
                .execute(conn)
                .expect("Error deleting journal entry");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = journal_entries::table
                .find(created_entry.id)
                .first::<JournalEntry>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_note_attributes_crud() {
        let conn = &mut establish_connection();

        conn.test_transaction(|conn| {
            // First create a note and attribute to work with
            let new_note = NewNote {
                title: "Test Note for Attributes",
                content: "This is a test note for attribute testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_note = diesel::insert_into(notes::table)
                .values(&new_note)
                .get_result::<Note>(conn)
                .expect("Error saving new note");

            let new_attribute = NewAttribute {
                name: "Test Attribute",
                description: Some("Test attribute description"),
            };

            let created_attribute = diesel::insert_into(attributes::table)
                .values(&new_attribute)
                .get_result::<Attribute>(conn)
                .expect("Error saving new attribute");

            // Test Create
            let new_note_attribute = NewNoteAttribute {
                note_id: Some(created_note.id),
                attribute_id: Some(created_attribute.id),
                value: "Test Value",
            };

            let created_note_attribute = diesel::insert_into(note_attributes::table)
                .values(&new_note_attribute)
                .get_result::<NoteAttribute>(conn)
                .expect("Error saving new note_attribute");

            dbg!(format!("Created Note Attribute #: {:?}", created_note_attribute.id));
            assert_eq!(created_note_attribute.note_id, Some(created_note.id));
            assert_eq!(created_note_attribute.attribute_id, Some(created_attribute.id));
            assert_eq!(created_note_attribute.value, "Test Value");

            // Test Read
            let read_note_attribute = note_attributes::table
                .find(created_note_attribute.id)
                .first::<NoteAttribute>(conn)
                .expect("Error loading note_attribute");

            assert_eq!(read_note_attribute.id, created_note_attribute.id);
            assert_eq!(read_note_attribute.note_id, created_note_attribute.note_id);
            assert_eq!(read_note_attribute.value, created_note_attribute.value);

            // Test Update
            let updated_note_attribute = diesel::update(note_attributes::table.find(created_note_attribute.id))
                .set(note_attributes::value.eq("Updated Value"))
                .get_result::<NoteAttribute>(conn)
                .expect("Error updating note_attribute");

            assert_eq!(updated_note_attribute.value, "Updated Value");

            dbg!(format!("Deleting Note Attribute #: {:?}", created_note_attribute.id));
            // Test Delete
            let deleted_count = diesel::delete(note_attributes::table.find(created_note_attribute.id))
                .execute(conn)
                .expect("Error deleting note_attribute");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = note_attributes::table
                .find(created_note_attribute.id)
                .first::<NoteAttribute>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_note_hierarchy_crud() {
        let conn = &mut establish_connection();

        conn.test_transaction(|conn| {
            // First create two notes to work with
            let new_parent_note = NewNote {
                title: "Parent Note",
                content: "This is a parent note for hierarchy testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_parent = diesel::insert_into(notes::table)
                .values(&new_parent_note)
                .get_result::<Note>(conn)
                .expect("Error saving parent note");

            let new_child_note = NewNote {
                title: "Child Note",
                content: "This is a child note for hierarchy testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_child = diesel::insert_into(notes::table)
                .values(&new_child_note)
                .get_result::<Note>(conn)
                .expect("Error saving child note");

            let hierarchy_type_var = "subpage";
            let hierarchy_type_var2 = "block";


            // Test Create
            let new_hierarchy = NewNoteHierarchy {
                parent_note_id: Some(created_parent.id),
                child_note_id: Some(created_child.id),
                hierarchy_type: Some(hierarchy_type_var),
            };

            let created_hierarchy = diesel::insert_into(note_hierarchy::table)
                .values(&new_hierarchy)
                .get_result::<NoteHierarchy>(conn)
                .expect("Error saving new note hierarchy");

            dbg!(format!("Created Note Hierarchy #: {:?}", created_hierarchy.id));
            assert_eq!(created_hierarchy.parent_note_id, Some(created_parent.id));
            assert_eq!(created_hierarchy.child_note_id, Some(created_child.id));
            assert_eq!(created_hierarchy.hierarchy_type, Some(hierarchy_type_var.to_string()));

            // Test Read
            let read_hierarchy = note_hierarchy::table
                .find(created_hierarchy.id)
                .first::<NoteHierarchy>(conn)
                .expect("Error loading note hierarchy");

            assert_eq!(read_hierarchy.id, created_hierarchy.id);
            assert_eq!(read_hierarchy.parent_note_id, created_hierarchy.parent_note_id);
            assert_eq!(read_hierarchy.child_note_id, created_hierarchy.child_note_id);

            // Test Update
            let updated_hierarchy = diesel::update(note_hierarchy::table.find(created_hierarchy.id))
                .set(note_hierarchy::hierarchy_type.eq(Some(hierarchy_type_var2)))
                .get_result::<NoteHierarchy>(conn)
                .expect("Error updating note hierarchy");

            assert_eq!(updated_hierarchy.hierarchy_type, Some(hierarchy_type_var2.to_string()));

            dbg!(format!("Deleting Note Hierarchy #: {:?}", created_hierarchy.id));
            // Test Delete
            let deleted_count = diesel::delete(note_hierarchy::table.find(created_hierarchy.id))
                .execute(conn)
                .expect("Error deleting note hierarchy");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = note_hierarchy::table
                .find(created_hierarchy.id)
                .first::<NoteHierarchy>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_note_modifications_crud() {
        let conn = &mut establish_connection();

        conn.test_transaction(|conn| {
            // First create a note to work with
            let new_note = NewNote {
                title: "Test Note for Modifications",
                content: "This is a test note for modifications testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_note = diesel::insert_into(notes::table)
                .values(&new_note)
                .get_result::<Note>(conn)
                .expect("Error saving new note");

            // Test Create
            let new_modification = NewNoteModification {
                note_id: Some(created_note.id),
                previous_content: "Original content",
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_modification = diesel::insert_into(note_modifications::table)
                .values(&new_modification)
                .get_result::<NoteModification>(conn)
                .expect("Error saving new note modification");

            dbg!(format!("Created Note Modification #: {:?}", created_modification.id));
            assert_eq!(created_modification.note_id, Some(created_note.id));
            assert_eq!(created_modification.previous_content, "Original content");

            // Test Read
            let read_modification = note_modifications::table
                .find(created_modification.id)
                .first::<NoteModification>(conn)
                .expect("Error loading note modification");

            assert_eq!(read_modification.id, created_modification.id);
            assert_eq!(read_modification.note_id, created_modification.note_id);
            assert_eq!(read_modification.previous_content, created_modification.previous_content);

            // Test Update
            let updated_modification = diesel::update(note_modifications::table.find(created_modification.id))
                .set(note_modifications::previous_content.eq("Updated content"))
                .get_result::<NoteModification>(conn)
                .expect("Error updating note modification");

            assert_eq!(updated_modification.previous_content, "Updated content");

            dbg!(format!("Deleting Note Modification #: {:?}", created_modification.id));
            // Test Delete
            let deleted_count = diesel::delete(note_modifications::table.find(created_modification.id))
                .execute(conn)
                .expect("Error deleting note modification");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = note_modifications::table
                .find(created_modification.id)
                .first::<NoteModification>(conn);

            assert!(matches!(find_result, Err(DieselError::NotFound)));

            Ok::<(), diesel::result::Error>(())
        });
    }

    #[test]
    fn test_note_type_mappings_crud() {
        let conn = &mut establish_connection();

        conn.test_transaction(|conn| {
            // First create a note and note type to work with
            let new_note = NewNote {
                title: "Test Note for Type Mapping",
                content: "This is a test note for type mapping testing",
                created_at: Some(chrono::Utc::now().naive_utc()),
                modified_at: Some(chrono::Utc::now().naive_utc()),
            };

            let created_note = diesel::insert_into(notes::table)
                .values(&new_note)
                .get_result::<Note>(conn)
                .expect("Error saving new note");

            let new_note_type = NewNoteType {
                name: "Test Note Type",
                description: Some("Test note type description"),
            };

            let created_note_type = diesel::insert_into(note_types::table)
                .values(&new_note_type)
                .get_result::<NoteType>(conn)
                .expect("Error saving new note type");

            // Test Create
            let new_type_mapping = NewNoteTypeMapping {
                note_id: created_note.id,
                type_id: created_note_type.id,
            };

            let created_mapping = diesel::insert_into(note_type_mappings::table)
                .values(&new_type_mapping)
                .get_result::<NoteTypeMapping>(conn)
                .expect("Error saving new note type mapping");

            assert_eq!(created_mapping.note_id, created_note.id);
            assert_eq!(created_mapping.type_id, created_note_type.id);

            // Test Read
            let read_mapping = note_type_mappings::table
                .filter(note_type_mappings::note_id.eq(created_note.id))
                .filter(note_type_mappings::type_id.eq(created_note_type.id))
                .first::<NoteTypeMapping>(conn)
                .expect("Error loading note type mapping");

            assert_eq!(read_mapping.note_id, created_mapping.note_id);
            assert_eq!(read_mapping.type_id, created_mapping.type_id);

            // Test Delete
            let deleted_count = diesel::delete(
                note_type_mappings::table
                    .filter(note_type_mappings::note_id.eq(created_note.id))
                    .filter(note_type_mappings::type_id.eq(created_note_type.id))
            )
                .execute(conn)
                .expect("Error deleting note type mapping");

            assert_eq!(deleted_count, 1);

            // Verify deletion
            let find_result = note_type_mappings::table
                .filter(note_type_mappings::note_id.eq(created_note.id))
                .filter(note_type_mappings::type_id.eq(created_note_type.id))
                .first::<NoteTypeMapping>(conn);

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

