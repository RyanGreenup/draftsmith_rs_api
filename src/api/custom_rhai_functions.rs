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
    let separator = "¶"; // This will be cloned into the closure below
    let sep2 = "$"; // The closure will take an immutable reference to this string
    let functions: Vec<CustomFn> = vec![
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
