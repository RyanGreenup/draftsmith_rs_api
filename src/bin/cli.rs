use clap::{Parser, Subcommand};
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use rust_cli_app::api;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Subcommand)]
enum AssetCommands {
    /// Create a new asset
    Create {
        /// Path to the file to upload
        file: PathBuf,
        /// Optional note ID to associate with
        #[arg(long)]
        note_id: Option<i32>,
        /// Optional description
        #[arg(long)]
        description: Option<String>,
        /// Optional custom filename
        #[arg(long)]
        filename: Option<String>,
    },
    /// List all assets
    List {
        /// Filter by note ID
        #[arg(long)]
        note_id: Option<i32>,
    },
    /// Get an asset by ID
    Get {
        /// Asset ID
        id: i32,
        /// Output file path
        output: PathBuf,
    },
    /// Get an asset by filename
    GetByName {
        /// Asset filename/path
        filename: String,
        /// Output file path
        output: PathBuf,
    },
    /// Update an asset
    Update {
        /// Asset ID
        id: i32,
        /// Optional note ID to associate with
        #[arg(long)]
        note_id: Option<i32>,
        /// Optional description
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete an asset
    Delete {
        /// Asset ID
        id: i32,
    },
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::ValueEnum, Clone)]
enum RenderType {
    Html,
    Md,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the API server
    Serve {
        /// The address to bind to
        #[arg(short, long, default_value = "127.0.0.1:37240")]
        addr: SocketAddr,
    },
    /// Client commands
    Client {
        /// The base URL of the API
        #[arg(long, default_value = rust_cli_app::BASE_URL)]
        url: String,
        #[command(subcommand)]
        command: ClientCommands,
    },
}

#[derive(Subcommand)]
enum ClientCommands {
    /// Notes related commands
    Notes {
        /// Optional note ID
        #[arg(long)]
        id: Option<i32>,
        #[command(subcommand)]
        command: NotesCommands,
    },
    /// Assets related commands
    Assets {
        #[command(subcommand)]
        command: AssetCommands,
    },
}

#[derive(Subcommand)]
enum NotesCommands {
    /// Clone all notes to a local directory
    Clone {
        /// Directory to save notes to
        dir: String,
    },
    /// Flat API commands
    Flat {
        #[command(subcommand)]
        command: FlatCommands,
    },
    /// Hierarchy commands
    Hierarchy {
        #[command(subcommand)]
        command: HierarchyCommands,
    },
    /// Display note tree
    Tree {
        /// Display simplified tree with only IDs and titles
        #[arg(long)]
        simple: bool,
    },
    /// Upload a new tree structure from JSON file
    Upload {
        /// Path to JSON file containing the tree structure
        file: String,
    },
    /// Push notes from a local directory to the server
    Push {
        /// Directory containing notes to push
        dir: String,
    },
    /// Render note content
    Render {
        /// Output file (optional - defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Render type (html or md)
        #[arg(short = 't', long, value_enum, default_value_t = RenderType::Md)]
        render_type: RenderType,
    },
    /// Full text search notes by content
    Fts {
        /// Search query
        query: String,
    },
}

#[derive(Subcommand)]
enum HierarchyCommands {
    /// Attach a note to a parent
    Attach {
        /// The parent note ID
        parent_id: i32,
    },
    /// Detach a note from its parent
    Detach,
    /// Show hierarchy mappings
    Mappings,
}

#[derive(Subcommand)]
enum FlatCommands {
    /// Get all notes
    Get {
        /// Only fetch metadata (exclude content)
        #[arg(long)]
        metadata_only: bool,
    },
    /// Create a new note
    Create {
        /// The content of the note
        #[arg(long)]
        content: String,
    },
    /// Update an existing note
    Update {
        /// The title of the note
        #[arg(long)]
        title: String,
        /// The content of the note
        #[arg(long)]
        content: String,
    },
    /// Delete a note
    Delete,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { addr } => {
            println!("Starting server on {}", addr);

            // Set up database connection pool
            let database_url =
                std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env file");
            let manager = ConnectionManager::<PgConnection>::new(database_url);
            let pool = r2d2::Pool::builder()
                .build(manager)
                .expect("Failed to create pool");

            // Create router with connection pool
            let app = api::create_router(pool);

            // Start server
            let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
            axum::serve(listener, app).tcp_nodelay(true).await.unwrap();
        }
        Commands::Client { url, command } => match command {
            ClientCommands::Notes { id, command } => match command {
                NotesCommands::Flat { command } => match command {
                    FlatCommands::Get { metadata_only } => {
                        if let Some(note_id) = id {
                            match rust_cli_app::client::fetch_note(&url, note_id, metadata_only)
                                .await
                            {
                                Ok(note) => {
                                    println!("{}", serde_json::to_string_pretty(&note).unwrap());
                                }
                                Err(rust_cli_app::client::NoteError::NotFound(id)) => {
                                    eprintln!("Error: Note with id {} not found", id);
                                    std::process::exit(1);
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            let notes = rust_cli_app::client::fetch_notes(&url, metadata_only)
                                .await
                                .unwrap();
                            println!("{}", serde_json::to_string_pretty(&notes).unwrap());
                        }
                    }
                    FlatCommands::Create { content } => {
                        let note = rust_cli_app::client::create_note(
                            &url,
                            rust_cli_app::client::CreateNoteRequest {
                                title: String::new(),
                                content,
                            },
                        )
                        .await
                        .unwrap();
                        println!("{}", serde_json::to_string_pretty(&note).unwrap());
                    }
                    FlatCommands::Update { title, content } => {
                        if let Some(note_id) = id {
                            match rust_cli_app::client::update_note(
                                &url,
                                note_id,
                                rust_cli_app::client::UpdateNoteRequest {
                                    title: Some(title),
                                    content,
                                },
                            )
                            .await
                            {
                                Ok(note) => {
                                    println!("{}", serde_json::to_string_pretty(&note).unwrap());
                                }
                                Err(rust_cli_app::client::NoteError::NotFound(id)) => {
                                    eprintln!("Error: Note with id {} not found", id);
                                    std::process::exit(1);
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            eprintln!("Error: --id is required for update command");
                            std::process::exit(1);
                        }
                    }
                    FlatCommands::Delete => {
                        if let Some(note_id) = id {
                            match rust_cli_app::client::delete_note(&url, note_id).await {
                                Ok(_) => {
                                    println!("Note {} deleted successfully", note_id);
                                }
                                Err(rust_cli_app::client::NoteError::NotFound(id)) => {
                                    eprintln!("Error: Note with id {} not found", id);
                                    std::process::exit(1);
                                }
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            eprintln!("Error: --id is required for delete command");
                            std::process::exit(1);
                        }
                    }
                },
                NotesCommands::Hierarchy { command } => match command {
                    HierarchyCommands::Attach { parent_id } => {
                        if let Some(child_id) = id {
                            let request = rust_cli_app::client::AttachChildRequest {
                                child_note_id: child_id,
                                parent_note_id: Some(parent_id),
                                hierarchy_type: Some("block".to_string()),
                            };
                            match rust_cli_app::client::attach_child_note(&url, request).await {
                                Ok(_) => println!(
                                    "Successfully attached note {} to parent {}",
                                    child_id, parent_id
                                ),
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            eprintln!("Error: --id is required for attach command");
                            std::process::exit(1);
                        }
                    }
                    HierarchyCommands::Detach => {
                        if let Some(child_id) = id {
                            match rust_cli_app::client::detach_child_note(&url, child_id).await {
                                Ok(_) => println!("Successfully detached note {}", child_id),
                                Err(e) => {
                                    eprintln!("Error: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        } else {
                            eprintln!("Error: --id is required for detach command");
                            std::process::exit(1);
                        }
                    }
                    HierarchyCommands::Mappings => {
                        match rust_cli_app::client::fetch_hierarchy_mappings(&url).await {
                            Ok(mappings) => {
                                println!("{}", serde_json::to_string_pretty(&mappings).unwrap());
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                std::process::exit(1);
                            }
                        }
                    }
                },
                NotesCommands::Tree { simple } => {
                    match rust_cli_app::client::fetch_note_tree(&url).await {
                        Ok(tree) => {
                            if simple {
                                print_simple_tree(&tree, 0);
                            } else {
                                println!("{}", serde_json::to_string_pretty(&tree).unwrap());
                            }
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                NotesCommands::Clone { dir } => {
                    // Create the directory if it doesn't exist
                    match std::fs::create_dir_all(&dir) {
                        Ok(_) => (),
                        Err(e) => {
                            eprintln!("Error creating directory {}: {}", dir, e);
                            std::process::exit(1);
                        }
                    }

                    // Fetch notes and tree
                    let notes = match rust_cli_app::client::fetch_notes(&url, false).await {
                        Ok(notes) => notes,
                        Err(e) => {
                            eprintln!("Error fetching notes: {}", e);
                            std::process::exit(1);
                        }
                    };

                    let tree = match rust_cli_app::client::fetch_note_tree(&url).await {
                        Ok(tree) => tree,
                        Err(e) => {
                            eprintln!("Error fetching note tree: {}", e);
                            std::process::exit(1);
                        }
                    };

                    // Download the notes
                    match rust_cli_app::client::write_notes_to_disk(
                        &notes,
                        &tree,
                        std::path::Path::new(&dir),
                    )
                    .await
                    {
                        Ok(_) => println!("Successfully cloned notes to {}", dir),
                        Err(e) => {
                            eprintln!("Error cloning notes: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                NotesCommands::Upload { file } => {
                    // Read the JSON file
                    let content = match std::fs::read_to_string(&file) {
                        Ok(content) => content,
                        Err(e) => {
                            eprintln!("Error reading file {}: {}", file, e);
                            std::process::exit(1);
                        }
                    };

                    // Parse the JSON into a NoteTreeNode, wrapping in Vec if needed
                    let tree: rust_cli_app::client::NoteTreeNode = match serde_json::from_str(
                        &content,
                    ) {
                        Ok(tree) => tree,
                        Err(e) => {
                            // Try parsing as array and take first element
                            match serde_json::from_str::<Vec<rust_cli_app::client::NoteTreeNode>>(
                                &content,
                            ) {
                                Ok(mut trees) if !trees.is_empty() => trees.remove(0),
                                Ok(_) => {
                                    eprintln!("Error: JSON file contains empty array");
                                    std::process::exit(1);
                                }
                                Err(_) => {
                                    eprintln!("Error parsing JSON: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                    };

                    // Upload the tree
                    match rust_cli_app::client::update_note_tree(&url, tree).await {
                        Ok(_) => println!("Tree structure updated successfully"),
                        Err(e) => {
                            eprintln!("Error updating tree: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                NotesCommands::Push { dir } => {
                    let dir_path = std::path::Path::new(&dir);
                    match rust_cli_app::client::read_from_disk(&url, dir_path).await {
                        Ok(_) => println!("Successfully pushed notes from {}", dir),
                        Err(e) => {
                            eprintln!("Error pushing notes: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                NotesCommands::Render {
                    output,
                    render_type,
                } => {
                    let rendered_output = if let Some(note_id) = id {
                        // Render single note
                        let rendered_content = match render_type {
                            RenderType::Html => {
                                match rust_cli_app::client::get_note_rendered_html(&url, note_id)
                                    .await
                                {
                                    Ok(html) => html,
                                    Err(rust_cli_app::client::NoteError::NotFound(id)) => {
                                        eprintln!("Error: Note with id {} not found", id);
                                        std::process::exit(1);
                                    }
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            RenderType::Md => {
                                match rust_cli_app::client::get_note_rendered_md(&url, note_id)
                                    .await
                                {
                                    Ok(md) => md,
                                    Err(rust_cli_app::client::NoteError::NotFound(id)) => {
                                        eprintln!("Error: Note with id {} not found", id);
                                        std::process::exit(1);
                                    }
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                        };
                        // For single note, create a JSON structure
                        serde_json::to_string_pretty(&rust_cli_app::client::RenderedNote {
                            id: note_id,
                            rendered_content,
                        })
                        .unwrap()
                    } else {
                        // Render all notes
                        let rendered_notes = match render_type {
                            RenderType::Html => {
                                match rust_cli_app::client::get_all_notes_rendered_html(&url).await
                                {
                                    Ok(notes) => notes,
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            RenderType::Md => {
                                match rust_cli_app::client::get_all_notes_rendered_md(&url).await {
                                    Ok(notes) => notes,
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                        };
                        // Convert to JSON
                        serde_json::to_string_pretty(&rendered_notes).unwrap()
                    };

                    // Write output to file or stdout
                    if let Some(output_path) = output {
                        match std::fs::write(output_path.clone(), rendered_output) {
                            Ok(_) => {
                                if let Some(note_id) = id {
                                    println!(
                                        "Rendered note {} to {}",
                                        note_id,
                                        output_path.display()
                                    );
                                } else {
                                    println!("Rendered all notes to {}", output_path.display());
                                }
                            }
                            Err(e) => {
                                eprintln!("Error writing to file {}: {}", output_path.display(), e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        println!("{}", rendered_output);
                    }
                }
                NotesCommands::Fts { query } => {
                    match rust_cli_app::client::fts_search_notes(&url, &query).await {
                        Ok(notes) => {
                            println!("{}", serde_json::to_string_pretty(&notes).unwrap());
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            },
            ClientCommands::Assets { command } => match command {
                AssetCommands::Create {
                    file,
                    note_id,
                    description,
                    filename,
                } => {
                    match rust_cli_app::client::create_asset(
                        &url,
                        &file,
                        note_id,
                        description,
                        filename,
                    )
                    .await
                    {
                        Ok(asset) => {
                            println!("{}", serde_json::to_string_pretty(&asset).unwrap());
                        }
                        Err(e) => {
                            eprintln!("Error creating asset: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                AssetCommands::List { note_id } => {
                    match rust_cli_app::client::list_assets(&url, note_id).await {
                        Ok(assets) => {
                            println!("{}", serde_json::to_string_pretty(&assets).unwrap());
                        }
                        Err(e) => {
                            eprintln!("Error listing assets: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                AssetCommands::Get { id, output } => {
                    match rust_cli_app::client::get_asset(&url, id, &output).await {
                        Ok(_) => {
                            println!("Asset downloaded to {}", output.display());
                        }
                        Err(rust_cli_app::client::NoteError::NotFound(_)) => {
                            eprintln!("Error: Asset not found");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("Error downloading asset: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                AssetCommands::GetByName { filename, output } => {
                    match rust_cli_app::client::get_asset_by_name(&url, &filename, &output).await {
                        Ok(_) => {
                            println!("Asset downloaded to {}", output.display());
                        }
                        Err(rust_cli_app::client::AssetError::FileNotFound(_)) => {
                            eprintln!("Error: Asset not found");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("Error downloading asset: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                AssetCommands::Update {
                    id,
                    note_id,
                    description,
                } => {
                    let update_request = rust_cli_app::client::UpdateAssetRequest {
                        note_id,
                        description,
                    };
                    match rust_cli_app::client::update_asset(&url, id, update_request).await {
                        Ok(asset) => {
                            println!("{}", serde_json::to_string_pretty(&asset).unwrap());
                        }
                        Err(rust_cli_app::client::AssetError::NotFound(_)) => {
                            eprintln!("Error: Asset not found");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("Error updating asset: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                AssetCommands::Delete { id } => {
                    match rust_cli_app::client::delete_asset(&url, id).await {
                        Ok(_) => {
                            println!("Asset {} deleted successfully", id);
                        }
                        Err(rust_cli_app::client::AssetError::NotFound(_)) => {
                            eprintln!("Error: Asset not found");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("Error deleting asset: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
            },
        },
    }
}

fn print_simple_tree(nodes: &[rust_cli_app::client::NoteTreeNode], depth: usize) {
    for node in nodes {
        // Print indentation
        print!("{}", "  ".repeat(depth));
        // Print node info in valid YAML format
        println!("{}:", node.id);
        // Print title with proper indentation
        print!("{}", "  ".repeat(depth + 1));
        println!(
            "title: {}",
            node.title.clone().expect("Node title should not be None")
        );
        // If there are children, print them as a nested list
        if !node.children.is_empty() {
            print!("{}", "  ".repeat(depth + 1));
            println!("children:");
            print_simple_tree(&node.children, depth + 2);
        }
    }
}

/*
CLI is not yet implemented, this is left as a reminder for later.
#[cfg(test)]
mod tests {
    use assert_cmd::Command;

    #[test]
    fn test_cli_with_name() {
        let mut cmd = Command::cargo_bin("cli").unwrap();
        cmd.arg("--name")
            .arg("Alice")
            .assert()
            .success()
            .stdout("Hello Alice!\n");
    }

    #[test]
    fn test_cli_with_name_and_count() {
        let mut cmd = Command::cargo_bin("cli").unwrap();
        cmd.arg("--name")
            .arg("Bob")
            .arg("--count")
            .arg("3")
            .assert()
            .success()
            .stdout("Hello Bob!\nHello Bob!\nHello Bob!\n");
    }

    #[test]
    fn test_cli_missing_name() {
        let mut cmd = Command::cargo_bin("cli").unwrap();
        cmd.assert().failure().stderr(predicates::str::contains(
            "error: the following required arguments were not provided:",
        ));
    }
}
*/
