use clap::{Parser, Subcommand};
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use rust_cli_app::api;
use serde_json::json;
use std::net::SocketAddr;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
}

#[derive(Subcommand)]
enum NotesCommands {
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
        /// The title of the note
        #[arg(long)]
        title: String,
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
            axum::serve(listener, app).await.unwrap();
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
                    FlatCommands::Create { title, content } => {
                        let note = rust_cli_app::client::create_note(
                            &url,
                            rust_cli_app::client::CreateNoteRequest { title, content },
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
                                rust_cli_app::client::UpdateNoteRequest { title, content },
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
                NotesCommands::Hierarchy { command } => {
                    if let Some(child_id) = id {
                        match command {
                            HierarchyCommands::Attach { parent_id } => {
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
                            }
                            HierarchyCommands::Detach => {
                                match rust_cli_app::client::detach_child_note(&url, child_id).await
                                {
                                    Ok(_) => println!("Successfully detached note {}", child_id),
                                    Err(e) => {
                                        eprintln!("Error: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                        }
                    } else {
                        eprintln!("Error: --id is required for hierarchy commands");
                        std::process::exit(1);
                    }
                }
                NotesCommands::Tree { simple } => match rust_cli_app::client::fetch_note_tree(&url).await {
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
                },
            },
        },
    }
}

fn print_simple_tree(nodes: &[rust_cli_app::client::NoteTreeNode], depth: usize) {
    for node in nodes {
        // Print indentation
        print!("{}", "  ".repeat(depth));
        // Print node info in YAML format
        println!("- {}:{} {}", node.id, if node.children.is_empty() { "" : " "}, node.title);
        // Recursively print children
        print_simple_tree(&node.children, depth + 1);
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
