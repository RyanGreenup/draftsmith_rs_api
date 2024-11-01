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

    /// Calls all CRUD tests in sequence.
    fn test_all() {
        Self::test_create();
        Self::test_read();
        Self::test_update();
        Self::test_delete();
    }
}

#[cfg(test)]
mod utils {
    use super::*;
    use diesel::pg::PgConnection;
    use dotenv::dotenv;
    use std::env;

    const ASSET_LOCATION: &str = "/test/path/exemplar_file.txt";

    pub fn establish_test_connection() -> PgConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&database_url)
            .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
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
    // TODO implement CrudTest trait for Asset struct
}

#[cfg(test)]
mod attributes {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Attribute struct
}

#[cfg(test)]
mod journalentrys {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Journalentry struct
}

#[cfg(test)]
mod noteattributes {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Noteattribute struct
}

#[cfg(test)]
mod notehierarchys {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Notehierarchy struct
}

#[cfg(test)]
mod notemodifications {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Notemodification struct
}

#[cfg(test)]
mod notetags {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Notetag struct
}

#[cfg(test)]
mod notetypemappings {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Notetypemapping struct
}

#[cfg(test)]
mod notetypes {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Notetype struct
}

#[cfg(test)]
mod notes {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Note struct
}

#[cfg(test)]
mod taghierarchys {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Taghierarchy struct
}

#[cfg(test)]
mod tags {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Tag struct
}

#[cfg(test)]
mod taskclocks {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Taskclock struct
}

#[cfg(test)]
mod taskschedules {
    use super::utils::*;
    use super::*;
    use crate::schema::assets::{self, table};
    use crate::schema::assets::dsl::*;
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    // TODO implement CrudTest trait for Taskschedule struct
}

#[cfg(test)]
mod tasks {
    use super::utils::*;
    use super::*;
    use crate::schema::tasks;
    use diesel::prelude::*;
    use chrono::NaiveDateTime;
    use bigdecimal::BigDecimal;
    use std::str::FromStr;

    impl CrudTest for Task {
        type Model = Task;

        fn test_create() {
            let conn = &mut establish_test_connection();
            
            let new_task = NewTask {
                note_id: Some(1),
                status: "NEW",
                effort_estimate: Some(BigDecimal::from_str("2.5").unwrap()),
                actual_effort: None,
                deadline: Some(NaiveDateTime::parse_from_str("2024-12-31 23:59:59", "%Y-%m-%d %H:%M:%S").unwrap()),
                priority: Some(1),
                created_at: Some(NaiveDateTime::parse_from_str("2024-11-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap()),
                modified_at: None,
                all_day: Some(false),
                goal_relationship: None,
            };

            let result = diesel::insert_into(tasks::table)
                .values(&new_task)
                .get_result::<Task>(conn)
                .expect("Error saving new task");

            assert_eq!(result.status, "NEW");
            assert_eq!(result.priority, Some(1));
        }

        fn test_read() {
            let conn = &mut establish_test_connection();
            
            let task = tasks::table
                .first::<Task>(conn)
                .expect("Error loading task");

            assert_eq!(task.status, "NEW");
        }

        fn test_update() {
            let conn = &mut establish_test_connection();
            
            let updated_rows = diesel::update(tasks::table.filter(tasks::id.eq(1)))
                .set(tasks::status.eq("IN_PROGRESS"))
                .execute(conn)
                .expect("Error updating task");

            assert_eq!(updated_rows, 1);

            let updated_task = tasks::table
                .find(1)
                .first::<Task>(conn)
                .expect("Error loading updated task");

            assert_eq!(updated_task.status, "IN_PROGRESS");
        }

        fn test_delete() {
            let conn = &mut establish_test_connection();
            
            let deleted_rows = diesel::delete(tasks::table.filter(tasks::id.eq(1)))
                .execute(conn)
                .expect("Error deleting task");

            assert_eq!(deleted_rows, 1);

            let result = tasks::table.find(1).first::<Task>(conn);
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_task_crud() {
        Task::test_all();
    }
}

