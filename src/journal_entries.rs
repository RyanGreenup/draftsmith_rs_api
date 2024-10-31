use crate::schema::journal_entries;
use crate::schema::journal_entries::dsl::*;
use chrono::NaiveDate;
use diesel::prelude::*;

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = journal_entries)]
pub struct JournalEntry {
    pub id: i32,
    pub note_id: Option<i32>,
    pub entry_date: NaiveDate,
}

#[derive(Insertable)]
#[diesel(table_name = journal_entries)]
pub struct NewJournalEntry {
    pub note_id: Option<i32>,
    pub entry_date: NaiveDate,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::{NewNote, Note};
    use crate::schema::notes::dsl::*;
    use crate::test_utils::establish_test_connection;
    use chrono::NaiveDate;
    use diesel::connection::Connection;

    fn create_test_note(conn: &mut PgConnection) -> Note {
        let new_note = NewNote {
            title: "Test Journal Note",
            content: "Test Content",
        };

        diesel::insert_into(notes)
            .values(&new_note)
            .get_result(conn)
            .expect("Error saving new note")
    }

    #[test]
    fn test_create_and_read_journal_entry() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a note first
            let test_note = create_test_note(conn);
            let today = chrono::Local::now().date_naive();

            // Create a new journal entry
            let new_entry = NewJournalEntry {
                note_id: Some(test_note.id),
                entry_date: today,
            };

            // Insert the journal entry
            let inserted_entry: JournalEntry = diesel::insert_into(journal_entries)
                .values(&new_entry)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_entry.note_id, Some(test_note.id));
            assert_eq!(inserted_entry.entry_date, today);

            // Read the journal entry back
            let found_entry = journal_entries
                .find(inserted_entry.id)
                .first::<JournalEntry>(conn)?;

            // Verify the read data
            assert_eq!(found_entry.id, inserted_entry.id);
            assert_eq!(found_entry.note_id, Some(test_note.id));
            assert_eq!(found_entry.entry_date, today);

            Ok(())
        });
    }

    #[test]
    fn test_update_journal_entry() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create initial note and journal entry
            let test_note = create_test_note(conn);
            let initial_date = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();

            let new_entry = NewJournalEntry {
                note_id: Some(test_note.id),
                entry_date: initial_date,
            };

            // Insert the journal entry
            let inserted_entry: JournalEntry = diesel::insert_into(journal_entries)
                .values(&new_entry)
                .get_result(conn)?;

            // Update to a new date
            let new_date = NaiveDate::from_ymd_opt(2024, 2, 1).unwrap();
            let updated_entry = diesel::update(journal_entries.find(inserted_entry.id))
                .set(entry_date.eq(new_date))
                .get_result::<JournalEntry>(conn)?;

            // Verify the update
            assert_eq!(updated_entry.entry_date, new_date);
            assert_eq!(updated_entry.note_id, Some(test_note.id));

            Ok(())
        });
    }

    #[test]
    fn test_delete_journal_entry() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            // Create a note and journal entry
            let test_note = create_test_note(conn);
            let today = chrono::Local::now().date_naive();

            let new_entry = NewJournalEntry {
                note_id: Some(test_note.id),
                entry_date: today,
            };

            // Insert the journal entry
            let inserted_entry: JournalEntry = diesel::insert_into(journal_entries)
                .values(&new_entry)
                .get_result(conn)?;

            // Delete the journal entry
            let deleted_count =
                diesel::delete(journal_entries.find(inserted_entry.id)).execute(conn)?;

            // Verify one record was deleted
            assert_eq!(deleted_count, 1);

            // Verify the journal entry no longer exists
            let find_result = journal_entries
                .find(inserted_entry.id)
                .first::<JournalEntry>(conn);
            assert!(find_result.is_err());

            Ok(())
        });
    }

    #[test]
    fn test_create_journal_entry_without_note() {
        let conn = &mut establish_test_connection();

        conn.test_transaction::<_, diesel::result::Error, _>(|conn| {
            let today = chrono::Local::now().date_naive();

            // Create a new journal entry without a note
            let new_entry = NewJournalEntry {
                note_id: None,
                entry_date: today,
            };

            // Insert the journal entry
            let inserted_entry: JournalEntry = diesel::insert_into(journal_entries)
                .values(&new_entry)
                .get_result(conn)?;

            // Verify the inserted data
            assert_eq!(inserted_entry.note_id, None);
            assert_eq!(inserted_entry.entry_date, today);

            Ok(())
        });
    }
}
