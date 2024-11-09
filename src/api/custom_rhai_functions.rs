use crate::tables::NoteWithoutFts;
use diesel::prelude::*;
use draftsmith_render::processor::{CustomFn, Processor};
use rhai::Engine;

pub fn build_custom_rhai_functions() -> Vec<CustomFn> {
    // Register custom functions
    fn double(x: i64) -> i64 {
        x * 2
    }
    fn concat(a: String, b: String) -> String {
        format!("{}{}", a, b)
    }

    fn transclusion(note_id: i64) -> String {
        let note_id = note_id as i32; // Rhai uses i64, Diesel uses i32
                                      // TODO this should be a public function: establish_connection
        dotenv::dotenv().ok();
        let database_url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
        let mut conn =
            PgConnection::establish(&database_url).expect("Error connecting to database");

        // This should be merged with crate::api::get_note
        use crate::schema::notes::dsl::*;
        let note = notes
            .find(note_id)
            .select(content)
            .first(&mut conn)
            .unwrap_or_else(|_| format!("Note with id {note_id} not found."));

        // TODO
        // Now parse this to html (This requires rethinking tbh, should transclusions involve pulling out a note and re-rendering it?)
        // How are recursive transclusions handled?
        // Should markdown be transcluded instead
        // But then those rhai blocks
        // NOTE
        // Probably register a different transclusion function for html
        // and markdown outputs.
        // This function could take an enum argument
        String::from(note)
    }

    let separator = "¶"; // This will be cloned into the closure below
    let sep2 = "$"; // The closure will take an immutable reference to this string
    let functions: Vec<CustomFn> = vec![
        Box::new(|engine: &mut Engine| {
            engine.register_fn("transclusion", transclusion);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("double", double);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("concat", concat);
        }),
        Box::new(move |engine: &mut Engine| {
            let separator = separator.to_string(); // Clone it here so we can move it into the next closure
            engine.register_fn("generate_ascii_diamond", move |size: i64| -> String {
                if size == 0 {
                    println!("Size must be greater than 0.");
                    return "".to_string();
                }

                let separator = format!("{separator}{sep2}");

                let separator = format!("{separator}{sep2}");

                let mut output = String::new();

                // Upper part of the diamond including the middle line
                for i in 0..size {
                    let spaces = " ".repeat((size - i) as usize);
                    let stars = separator.repeat((2 * i + 1) as usize);
                    let line = format!("{spaces}{stars}\n");
                    output.push_str(&line);
                }

                // Lower part of the diamond
                for i in (0..size - 1).rev() {
                    let spaces = " ".repeat((size - i) as usize);
                    let stars = separator.repeat((2 * i + 1) as usize);
                    let line = format!("{spaces}{stars}\n");
                    output.push_str(&line);
                }
                format!("<pre>\n{}\n</pre>", output)
            });
        }),
    ];

    functions
}

pub fn parse_md_to_html(document: &str) -> String {
    let functions = build_custom_rhai_functions();
    draftsmith_render::parse_md_to_html(document, Some(functions))
}

pub fn process_md(document: &str) -> String {
    let functions = build_custom_rhai_functions();
    let mut processor = Processor::new(Some(functions));
    processor.process(document)
}
