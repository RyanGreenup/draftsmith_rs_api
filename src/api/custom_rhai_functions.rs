use crate::api::get_note_content;
use diesel::prelude::*;
use draftsmith_render::processor::{CustomFn, Processor};
use lazy_static::lazy_static;
use rhai::{Engine, ImmutableString};
use std::sync::Mutex;

// enum for html vs markdown
enum RenderTarget {
    Html,
    Markdown,
}

// Defines a global, mutable vector protected by a Mutex for tracking recursion path
lazy_static! {
    static ref RECURSION_PATH: Mutex<Vec<i64>> = Mutex::new(Vec::new());
}

// RAII guard for managing recursion stack
struct RecursionGuard {
    note_id: i64,
}

impl RecursionGuard {
    fn new(note_id: i64) -> Option<Self> {
        let mut vec = RECURSION_PATH
            .lock()
            .expect("Failed to lock recursion vector");
        if vec.contains(&note_id) {
            // Found recursion - leave the vector as is so we can show the full path
            None
        } else {
            vec.push(note_id);
            Some(Self { note_id })
        }
    }
}

impl Drop for RecursionGuard {
    fn drop(&mut self) {
        let mut vec = RECURSION_PATH
            .lock()
            .expect("Failed to lock recursion vector");
        if let Some(pos) = vec.iter().position(|&x| x == self.note_id) {
            vec.truncate(pos); // Remove this and all subsequent items
        }
    }
}

fn build_custom_rhai_functions(render_target: RenderTarget) -> Vec<CustomFn> {
    // Register custom functions
    fn double(x: i64) -> i64 {
        x * 2
    }
    fn concat(a: ImmutableString, b: ImmutableString) -> String {
        format!("{}{}", a, b)
    }

    // TODO this should take a css from the site
    fn thumbnail(filename: &str, title: &str, description: &str) -> String {
        let div = format!(
            r#"
<style>
    .ds-float-right-clear {{
      float: right;
      clear: left;
    }}
</style>
<div class="card card-compact bg-base-100 w-80 shadow-xl ds-float-right-clear">
    <figure>
        <img
            src="/m/{filename}"
            alt="{filename}" />
    </figure>
    <div class="card-body">
        <h3 class="card-title">{title}</h3>
            <p>{description}</p>
    </div>
</div>"#,
            filename = filename,
            title = title,
            description = description
        );
        div
    }

    fn radial_progress(percentage: i64) -> String {
        assert!(percentage <= 100, "Percentage must be between 0 and 100");

        format!(
            r#"<div class="radial-progress" style="--value:{};" role="progressbar">{}%</div>"#,
            percentage, percentage
        )
    }

    fn rating_stars(rating: i64) -> String {
        assert!(rating <= 5, "Rating must be between 0 and 5");

        let mut stars_html = String::from(r#"<div class="rating">"#);

        for i in 0..5 {
            if i < rating {
                stars_html.push_str(r#"<input type="radio" name="rating-1" class="mask mask-star" checked="checked" />"#);
            } else {
                stars_html
                    .push_str(r#"<input type="radio" name="rating-1" class="mask mask-star" />"#);
            }
        }

        stars_html.push_str("</div>");
        stars_html
    }

    fn transclusion_to_md(note_id: i64) -> String {
        match RecursionGuard::new(note_id) {
            None => {
                let vec = RECURSION_PATH
                    .lock()
                    .expect("Failed to lock recursion vector");
                let path = vec
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(" → ");
                format!(
                    "<div class='bg-red-100 p-2'>Recursion detected: {} → {}</div>",
                    path, note_id
                )
            }
            Some(_guard) => {
                let content = get_note_content(note_id as i32);
                process_md(&content)
            }
        }
    }

    fn transclusion_to_html(note_id: i64) -> String {
        match RecursionGuard::new(note_id) {
            None => {
                let vec = RECURSION_PATH
                    .lock()
                    .expect("Failed to lock recursion vector");
                let path = vec
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(" → ");
                format!(
                    "<div class='bg-red-100 p-2'>Recursion detected: {} → {}</div>",
                    path, note_id
                )
            }
            Some(_guard) => {
                let content = get_note_content(note_id as i32);
                parse_md_to_html(&content)
            }
        }
    }

    let separator = "¶"; // This will be cloned into the closure below
    let sep2 = "$"; // The closure will take an immutable reference to this string
    let mut functions: Vec<CustomFn> = vec![
        Box::new(|engine: &mut Engine| {
            engine.register_fn("thumbnail", thumbnail);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("double", double);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("concat", concat);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("rating_stars", rating_stars);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("radial_progress", radial_progress);
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

    match render_target {
        RenderTarget::Html => functions.append(&mut vec![Box::new(|engine: &mut Engine| {
            engine.register_fn("transclusion", transclusion_to_html);
        })]),
        RenderTarget::Markdown => functions.append(&mut vec![Box::new(|engine: &mut Engine| {
            engine.register_fn("transclusion", transclusion_to_md);
        })]),
    }

    functions
}

pub fn parse_md_to_html(document: &str) -> String {
    let functions = build_custom_rhai_functions(RenderTarget::Html);
    draftsmith_render::parse_md_to_html(document, Some(functions))
}

pub fn process_md(document: &str) -> String {
    let functions = build_custom_rhai_functions(RenderTarget::Markdown);
    let mut processor = Processor::new(Some(functions));
    processor.process(document)
}
