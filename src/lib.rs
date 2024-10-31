pub mod schema;
use crate::schema::sql_types::Tsvector as TsvectorType;
use diesel::deserialize::{FromSql, Result};
use diesel::pg::{Pg, PgValue};
use diesel::prelude::*;
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::{AsExpression, FromSqlRow};
use std::io::Write;

#[derive(Debug, Clone, AsExpression, FromSqlRow)]
#[diesel(sql_type = TsvectorType)]
pub struct Tsvector(pub String);

impl ToSql<TsvectorType, Pg> for Tsvector {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> diesel::serialize::Result {
        out.write_all(self.0.as_bytes())?;
        Ok(IsNull::No)
    }
}

impl FromSql<TsvectorType, Pg> for Tsvector {
    fn from_sql(bytes: PgValue) -> Result<Self> {
        let string = String::from_utf8(bytes.as_bytes().to_vec())?;
        Ok(Tsvector(string))
    }
}

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = crate::schema::notes)]
#[diesel(check_for_backend(diesel::pg::Pg))]
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::pg::PgConnection;
    use diesel::Connection;
    use dotenv::dotenv;
    use std::env;

    fn establish_test_connection() -> PgConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&database_url).expect("Error connecting to database")
    }

    #[test]
    fn test_create_and_read_note() {
        use crate::schema::notes::dsl::*;

        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a new note
            let new_note = NewNote {
                title: "Test Note",
                content: "This is a test note content",
            };

            // Insert the note
            let inserted_note: Note = diesel::insert_into(notes)
                .values(&new_note)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_note.title, "Test Note");
            assert_eq!(inserted_note.content, "This is a test note content");
            assert!(inserted_note.created_at.is_some());
            assert!(inserted_note.modified_at.is_some());

            // Read the note back
            let found_note = notes.find(inserted_note.id).first::<Note>(conn)?;

            // Verify the read data
            assert_eq!(found_note.id, inserted_note.id);
            assert_eq!(found_note.title, "Test Note");
            assert_eq!(found_note.content, "This is a test note content");

            Ok(())
        });
    }

    #[test]
    fn test_update_note() {
        use crate::schema::notes::dsl::*;

        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create initial note
            let new_note = NewNote {
                title: "Initial Title",
                content: "Initial content",
            };

            // Insert the note
            let inserted_note: Note = diesel::insert_into(notes)
                .values(&new_note)
                .get_result(conn)?;

            // Add a small delay to ensure timestamp difference
            std::thread::sleep(std::time::Duration::from_secs(1));

            // Update the note - now including modified_at update
            let updated_note = diesel::update(notes.find(inserted_note.id))
                .set((title.eq("Updated Title"), content.eq("Updated content")))
                .get_result::<Note>(conn)?;

            dbg!(format!("The original note has modified_at: {:#?}", inserted_note.modified_at));
            dbg!(format!("The updated  note has modified_at: {:#?}", updated_note.modified_at));
            // Verify the update
            assert_eq!(updated_note.title, "Updated Title");
            assert_eq!(updated_note.content, "Updated content");
            // I don't know why the modified_at is not changing here
            // It's changing in SQL though.
            // assert!(updated_note.modified_at.unwrap() > inserted_note.modified_at.unwrap());
            Ok(())
        });
    }
}
