pub mod assets;
pub mod attributes;
pub mod journal_entries;
pub mod note_attributes;
pub mod note_hierarchy;
pub mod note_modifications;
pub mod note_tags;
pub mod note_type_mappings;
pub mod note_types;
pub mod notes;
pub mod schema;
pub mod tag_hierarchy;
pub mod tags;

use diesel::deserialize::{FromSql, Result};
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::{AsExpression, FromSqlRow};
use std::io::Write;
use diesel::prelude::*;


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

#[cfg(test)]
pub mod test_utils {
    use diesel::pg::PgConnection;
    use diesel::Connection;
    use dotenv::dotenv;
    use std::env;
    use crate::schema::tags;
    use crate::tags::NewTag;
    use crate::tags::Tag;
    use crate::schema::tags::dsl::*;

    pub fn establish_test_connection() -> PgConnection {
        dotenv().ok();
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        PgConnection::establish(&database_url).expect("Error connecting to database")
    }

    fn setup_default_data(conn: &mut PgConnection) -> Result<(), diesel::result::Error> {
        // Insert some default tags into the database
        let new_tags = vec![NewTag { name: "Tag1" }, NewTag { name: "Tag2" }];
        let mut inserted_tags = Vec::with_capacity(new_tags.len());

        for new_tag in new_tags.iter() {
            // Insert the tag
            let inserted_tag: Tag = diesel::insert_into(tags)
                .values(new_tag.clone())
                .get_result(conn)?;
        }

        Ok(())
    }
}
