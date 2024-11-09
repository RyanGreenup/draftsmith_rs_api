use crate::api::get_note_content;
use crate::tables::NoteWithoutFts;
use diesel::prelude::*;
use draftsmith_render::processor::{CustomFn, Processor};
use rhai::Engine;

// enum for html vs markdown
enum RenderTarget {
    Html,
    Markdown,
}

pub fn build_custom_rhai_functions(render_target: RenderTarget) -> Vec<CustomFn> {
    // Register custom functions
    fn double(x: i64) -> i64 {
        x * 2
    }
    fn concat(a: String, b: String) -> String {
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
<div class="card card-compact bg-base-100 w-1/6 shadow-xl ds-float-right-clear">
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

    // TODO how to deal with recursion
    // TODO consider a card class
    fn transclusion_to_md(note_id: i64) -> String {
        let content = get_note_content(note_id as i32);
        process_md(&content)
    }

    fn transclusion_to_html(note_id: i64) -> String {
        let content = get_note_content(note_id as i32);
        parse_md_to_html(&content)
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
