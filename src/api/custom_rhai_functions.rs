use crate::api::{get_note_content, get_note_title};
use draftsmith_render::processor::{CustomFn, Processor};
use glob::glob;
use lazy_static::lazy_static;
use rhai::{Array, Dynamic, Engine, ImmutableString};
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

    fn radial_progress(percentage: i64) -> String {
        assert!(percentage <= 100, "Percentage must be between 0 and 100");

        format!(
            r#"<div class="radial-progress" style="--value:{};" role="progressbar">{}%</div>"#,
            percentage, percentage
        )
    }

    fn remap_key(val: &str) -> &str {
        match val {
            "C" => "Ctrl",
            "A" => "Alt",
            "S" => "Shift",
            "s" => "🐧",
            "M" => "Alt", // Assuming "M" should map to "Meta" or possibly "Alt"
            "F1" | "F2" | "F3" | "F4" | "F5" | "F6" | "F7" | "F8" | "F9" | "F10" | "F11"
            | "F12" => val,
            "Home" | "End" | "PageUp" | "PageDown" | "Insert" | "Delete" | "Tab" | "Enter"
            | "Esc" => val,
            _ => val,
        }
    }

    fn keyboard_shortcut_to_kbd_html(input: &str) -> String {
        let start = r#"<kbd class="kbd">"#;
        let end = r#"</kbd>"#;
        let mut output = String::new();

        let shortcuts: Vec<&str> = input.split('-').collect();
        let num_shortcuts = shortcuts.len();

        for (i, shortcut) in shortcuts.iter().enumerate() {
            let mapped = remap_key(shortcut);
            output.push_str(start);
            output.push_str(mapped);
            output.push_str(end);

            // Append '+' between kbd elements, except after the last one
            if i < num_shortcuts - 1 {
                output.push_str("+");
            }
        }

        output
    }

    fn generate_diff_html(before: &str, after: &str) -> String {
        format!(
            r#"
<div class="diff aspect-[16/9]">
  <div class="diff-item-1">
    {before}
  </div>
  <div class="diff-item-2">
    {after}
  </div>
  <div class="diff-resizer"></div>
</div>
"#,
            before = before,
            after = after
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

    fn link(note_id: i64) -> String {
        let title = get_note_title(note_id as i32);
        // TODO is flask /note, what about Qt? How are wikilinks handled?
        match title {
            Err(e) => format!("Note not found: {}. Error: {e}", note_id),
            Ok(title) => format!("[{}](/note/{})", title, note_id),
        }
    }

    fn handle_recursion(note_id: i64) -> Option<String> {
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
                Some(format!(
                    "<div class='bg-red-100 p-2'>Recursion detected: {} → {}</div>",
                    path, note_id
                ))
            }
            Some(_guard) => None,
        }
    }

    fn process_content(note_id: i64, processor: fn(&str) -> String) -> String {
        if note_id > 0 {
            match get_note_content(note_id as i32) {
                Ok(content) => processor(&content),
                Err(e) => format!("Error fetching note content: {}", e),
            }
        } else {
            format!("ID must be > 0, id: {} invalid", note_id)
        }
    }

    fn transclusion_to_md(note_id: i64) -> String {
        if let Some(recursion_msg) = handle_recursion(note_id) {
            return recursion_msg;
        }
        process_content(note_id, |content| process_md(content))
    }

    fn transclusion_to_html(note_id: i64) -> String {
        if let Some(recursion_msg) = handle_recursion(note_id) {
            return recursion_msg;
        }
        process_content(note_id, |content| parse_md_to_html(content))
    }

    fn image(src: &str, width: i64, alt: &str) -> String {
        format!(
            r#"<p><img src="/m/{src}" style="width:{width}%" alt="{alt}" /></p>"#,
            src = src,
            width = width,
            alt = alt
        )
    }

    fn figure(image_html: ImmutableString, caption: &str, float: bool) -> String {
        let mut div = r#"<div class="card card-compact bg-base-100 w-80 shadow-xl">"#;
        if float {
            div = r#"<div class="card card-compact bg-base-100 w-80 shadow-xl" style="float: right; clear: left;">"#;
        }
        format!(
            r#"
{}
<figure>
{}
</figure>
<div class="card-body">
<p>{}</p>
</div>
</div>"#,
            div, image_html, caption,
        )
    }

    // TODO this should take a css from the site and asign the div class
    fn thumbnail(filename: &str, title: &str, description: &str) -> String {
        let div = format!(
            r#"
<div class="card card-compact bg-base-100 w-80 shadow-xl" style="float: right; clear: left">
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

    fn video_player_html(video_filename: &str) -> String {
        format!(
            r#"
<details open><summary>📼</summary>
<div class="max-w-xl mx-auto p-4 border border-gray-300 rounded-lg shadow-md resize overflow-auto">
<video class="w-full h-auto" controls>
<source src="/m/{filename}" type="video/mp4">
</video>
</div>
</details>
    "#,
            filename = video_filename
        )
    }

    fn list_assets(pattern: &str) -> Dynamic {
        let pattern = format!("./uploads/{}", pattern);
        let mut files = Vec::new();

        // Attempt to get entries from the glob pattern
        let entries = match glob(&pattern) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Error processing glob pattern: {}", e);
                return Array::new().into(); // Return an empty array on error
            }
        };

        for entry in entries {
            match entry {
                Ok(path) => {
                    if let Some(path_str) = path.to_str() {
                        // Convert path to string and remove "./uploads/" prefix
                        files.push(ImmutableString::from(path_str.replace("uploads/", "")));
                    } else {
                        eprintln!("Error converting path to string for: {:?}", path);
                    }
                }
                Err(e) => eprintln!("Error processing an entry: {}", e),
            }
        }

        files.into() // Return the array wrapped in Dynamic
    }

    fn gallery(title: ImmutableString, images: Array) -> String {
        let mut out = String::new();

        for im in images {
            let image_src = format!("/m/{}", im);
            out.push_str("<div><img src=\"");
            out.push_str(&image_src);
            out.push_str("\" class=\"gallery-image\" /></div>");
        }

        let div_start = format!(
            r#"
<div class="max-w-4xl mx-auto p-6 border border-gray-200 rounded-lg shadow-md">
<h2 class="text-2xl font-bold">{}</h2>
    <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
"#,
            title
        );
        let div_end = r#"
    </div>
</div>
"#;

        [div_start, out, div_end.to_string()].concat().to_string()
    }

    #[allow(unused_assignments)]
    fn timeline(events: Array) -> String {
        let mut html_output = String::from(
            r#"<ul class="timeline timeline-snap-icon max-md:timeline-compact timeline-vertical">"#,
        );

        #[allow(clippy::needless_range_loop)]
        for i in 0..events.len() {
            let mut year = ImmutableString::new();
            let mut title = ImmutableString::new();
            let mut description = ImmutableString::new();
            match events[i].clone().into_array() {
                Ok(event) => {
                    match event[0].clone().into_immutable_string() {
                        Ok(y) => {
                            year = y;
                        }
                        Err(e) => {
                            return format!("Error parsing events: {e}");
                        }
                    }
                    match event[1].clone().into_immutable_string() {
                        Ok(s) => {
                            title = s;
                        }
                        Err(e) => {
                            return format!("Error parsing events: {e}");
                        }
                    }
                    match event[2].clone().into_immutable_string() {
                        Ok(s) => {
                            description = s;
                        }
                        Err(e) => {
                            return format!("Error parsing events: {e}");
                        }
                    }
                }
                Err(_) => {
                    return "Error parsing events".to_string();
                }
            }
            html_output.push_str(&format!(
            r#"<li>
    <div class="timeline-middle">
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-5 w-5">
            <path fill-rule="evenodd" d="M10 18a8 8 0 100-16 8 8 0 000 16zm3.857-9.809a.75.75 0 00-1.214-.882l-3.483 4.79-1.88-1.88a.75.75 0 10-1.06 1.061l2.5 2.5a.75.75 0 001.137-.089l4-5.5z" clip-rule="evenodd" />
        </svg>
    </div>
    <div class="timeline-start mb-10 md:text-end">
        <time class="font-mono italic">{}</time>
        <div class="text-lg font-black">{}</div>
        {}
    </div>
    <hr />
</li>"#,
            year, title, description
        ));
        }

        html_output.push_str("</ul>");
        html_output
    }

    /*
        fn timeline(years: Array) -> String {
            // let years = vec!["1984", "1998"];
            let titles = vec!["First Macintosh computer", "iMac"];
            let descriptions = vec![
            "The Apple Macintosh—later rebranded as the Macintosh 128K—is the original Apple Macintosh personal computer. It played a pivotal role in establishing desktop publishing as a general office function. The motherboard, a 9 in (23 cm) CRT monitor, and a floppy drive were housed in a beige case with integrated carrying handle; it came with a keyboard and single-button mouse.",
            "iMac is a family of all-in-one Mac desktop computers designed and built by Apple Inc. It has been the primary part of Apple's consumer desktop offerings since its debut in August 1998, and has evolved through seven distinct forms.",
        ];

            let timeline = timeline_html(years, &titles, &descriptions);
            timeline
        }
    */

    let separator = "¶"; // This will be cloned into the closure below
    let sep2 = "$"; // The closure will take an immutable reference to this string
    let mut functions: Vec<CustomFn> = vec![
        Box::new(|engine: &mut Engine| {
            engine.register_fn("diff_display", generate_diff_html);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("kbd", keyboard_shortcut_to_kbd_html);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("video", video_player_html);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("list_assets", list_assets);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("gallery", gallery);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("timeline", timeline);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("figure", figure);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("image", image);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("thumbnail", thumbnail);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("double", double);
        }),
        Box::new(|engine: &mut Engine| {
            engine.register_fn("link", link);
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
