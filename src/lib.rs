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
