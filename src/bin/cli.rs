use bigdecimal::BigDecimal;
use chrono::NaiveDateTime;
use clap::{Parser, Subcommand};
use diesel::pg::PgConnection;
use diesel::r2d2::{self, ConnectionManager};
use rust_cli_app::client::tags::{
    self, attach_child_tag, create_tag, delete_tag, detach_child_tag, get_hierarchy_mappings,
    get_tag, list_tags, update_tag, CreateTagRequest, TagError, UpdateTagRequest,
};
use rust_cli_app::client::tasks::{
    attach_child_task, create_task, delete_task, detach_child_task, fetch_task, fetch_task_tree,
    fetch_tasks, update_task, AttachChildRequest, CreateTaskRequest, TaskError, TaskTreeNode,
    UpdateTaskRequest,
};
use rust_cli_app::client::NoteTreeNode;
use rust_cli_app::{api, client::tasks::*};
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
    /// Get an asset by filename/path
    GetByName {
        /// Asset path (can include directories, e.g. 'dir1/dir2/file.txt')
        #[arg(value_name = "PATH")]
        path: String,
        /// Output file path
        #[arg(value_name = "OUTPUT")]
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
enum TasksCommands {
    /// List all tasks
    List,
    /// Get a task by ID
    Get,
    /// Create a new task
    Create {
        /// Status of the task (e.g., todo, in_progress, done)
        #[arg(long)]
        status: String,
        /// Optional note ID associated with the task
        #[arg(long)]
        note_id: Option<i32>,
        /// Optional effort estimate in hours
        #[arg(long)]
        effort_estimate: Option<i32>,
        /// Optional actual effort in hours
        #[arg(long)]
        actual_effort: Option<i32>,
        /// Optional deadline in ISO 8601 format (e.g., 2023-12-31T23:59:59)
        #[arg(long)]
        deadline: Option<String>,
        /// Optional priority level
        #[arg(long)]
        priority: Option<i32>,
        /// Whether the task is an all-day task
        #[arg(long)]
        all_day: Option<bool>,
        /// Optional goal relationship description
        #[arg(long)]
        goal_relationship: Option<String>,
    },
    /// Update an existing task
    Update {
        /// Status of the task (e.g., todo, in_progress, done)
        #[arg(long)]
        status: Option<String>,
        /// Optional note ID associated with the task
        #[arg(long)]
        note_id: Option<i32>,
        /// Optional effort estimate in hours
        #[arg(long)]
        effort_estimate: Option<i32>,
        /// Optional actual effort in hours
        #[arg(long)]
        actual_effort: Option<i32>,
        /// Optional deadline in ISO 8601 format (e.g., 2023-12-31T23:59:59)
        #[arg(long)]
        deadline: Option<String>,
        /// Optional priority level
        #[arg(long)]
        priority: Option<i32>,
        /// Whether the task is an all-day task
        #[arg(long)]
        all_day: Option<bool>,
        /// Optional goal relationship description
        #[arg(long)]
        goal_relationship: Option<String>,
    },
    /// Delete a task
    Delete,
    /// Fetch the task tree
    Tree {
        /// Display simplified tree with only IDs and statuses
        #[arg(long)]
        simple: bool,
    },
    /// Show hierarchy mappings
    Mappings,
    /// Attach a task to a parent task
    Attach {
        /// The parent task ID
        parent_id: i32,
    },
    /// Detach a task from its parent
    Detach,
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
enum TagsCommands {
    /// Create a new tag
    Create {
        /// Name of the tag
        #[arg(long)]
        name: String,
    },
    /// List all tags
    List,
    /// Get a tag by ID
    Get {
        /// ID of the tag to retrieve
        id: i32,
    },
    /// Update an existing tag
    Update {
        /// ID of the tag to update
        id: i32,
        /// New name for the tag
        #[arg(long)]
        name: String,
    },
    /// Delete a tag
    Delete {
        /// ID of the tag to delete
        id: i32,
    },
    /// Display tag tree
    Tree {
        /// Display simplified tree with only IDs and names
        #[arg(long)]
        simple: bool,
    },
    /// Show hierarchy mappings
    Mappings,
    /// Attach a tag to a parent tag
    Attach {
        /// The parent tag ID
        #[arg(long)]
        parent_id: i32,
        /// The child tag ID
        #[arg(long)]
        child_id: i32,
    },
    /// Detach a tag from its parent
    Detach {
        /// The child tag ID to detach
        #[arg(long)]
        child_id: i32,
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
    /// Tasks related commands
    Tasks {
        /// Optional task ID
        #[arg(long)]
        id: Option<i32>,
        #[command(subcommand)]
        command: TasksCommands,
    },
    /// Tags related commands
    Tags {
        #[command(subcommand)]
        command: TagsCommands,
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
                    file: _,
                    note_id: _,
                    description: _,
                    filename: _,
                } => {
                    // TODO: Implement asset creation logic
                    eprintln!("Asset creation not yet implemented");
                    std::process::exit(1);
                }
                AssetCommands::List { note_id: _ } => {
                    // TODO: Implement asset listing logic
                    eprintln!("Asset listing not yet implemented");
                    std::process::exit(1);
                }
                AssetCommands::Get { id: _, output: _ } => {
                    // TODO: Implement get asset logic
                    eprintln!("Asset retrieval not yet implemented");
                    std::process::exit(1);
                }
                AssetCommands::GetByName { path: _, output: _ } => {
                    // TODO: Implement get by name logic
                    eprintln!("Asset retrieval by name not yet implemented");
                    std::process::exit(1);
                }
                AssetCommands::Update {
                    id: _,
                    note_id: _,
                    description: _,
                } => {
                    // TODO: Implement update logic
                    eprintln!("Asset update not yet implemented");
                    std::process::exit(1);
                }
                AssetCommands::Delete { id: _ } => {
                    // TODO: Implement delete logic
                    eprintln!("Asset deletion not yet implemented");
                    std::process::exit(1);
                }
            },
            ClientCommands::Tags { command } => match command {
                TagsCommands::Create { name } => {
                    let request = CreateTagRequest { name };
                    match create_tag(&url, request).await {
                        Ok(tag) => {
                            println!("{}", serde_json::to_string_pretty(&tag).unwrap());
                        }
                        Err(e) => {
                            eprintln!("Error creating tag: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                TagsCommands::List => match list_tags(&url).await {
                    Ok(tags) => {
                        println!("{}", serde_json::to_string_pretty(&tags).unwrap());
                    }
                    Err(e) => {
                        eprintln!("Error listing tags: {}", e);
                        std::process::exit(1);
                    }
                },
                TagsCommands::Get { id } => match get_tag(&url, id).await {
                    Ok(tag) => {
                        println!("{}", serde_json::to_string_pretty(&tag).unwrap());
                    }
                    Err(TagError::NotFound) => {
                        eprintln!("Error: Tag with id {} not found", id);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                TagsCommands::Update { id, name } => {
                    let request = UpdateTagRequest { name };
                    match update_tag(&url, id, request).await {
                        Ok(tag) => {
                            println!("{}", serde_json::to_string_pretty(&tag).unwrap());
                        }
                        Err(TagError::NotFound) => {
                            eprintln!("Error: Tag with id {} not found", id);
                            std::process::exit(1);
                        }
                        Err(e) => {
                            eprintln!("Error: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                TagsCommands::Delete { id } => match delete_tag(&url, id).await {
                    Ok(_) => {
                        println!("Tag {} deleted successfully", id);
                    }
                    Err(TagError::NotFound) => {
                        eprintln!("Error: Tag with id {} not found", id);
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                TagsCommands::Tree { simple: _ } => {
                    eprintln!("Tag tree functionality not yet implemented");
                    std::process::exit(1);
                }
                TagsCommands::Mappings => match get_hierarchy_mappings(&url).await {
                    Ok(mappings) => {
                        println!("{}", serde_json::to_string_pretty(&mappings).unwrap());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                TagsCommands::Attach {
                    parent_id,
                    child_id,
                } => match attach_child_tag(&url, parent_id, child_id).await {
                    Ok(_) => {
                        println!(
                            "Successfully attached tag {} to parent {}",
                            child_id, parent_id
                        );
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                TagsCommands::Detach { child_id } => match detach_child_tag(&url, child_id).await {
                    Ok(_) => {
                        println!("Successfully detached tag {}", child_id);
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
            },
            ClientCommands::Tasks { id, command } => match command {
                TasksCommands::List => match fetch_tasks(&url).await {
                    Ok(tasks) => {
                        println!("{}", serde_json::to_string_pretty(&tasks).unwrap());
                    }
                    Err(e) => {
                        eprintln!("Error fetching tasks: {}", e);
                        std::process::exit(1);
                    }
                },
                TasksCommands::Get => {
                    if let Some(task_id) = id {
                        match fetch_task(&url, task_id).await {
                            Ok(task) => {
                                println!("{}", serde_json::to_string_pretty(&task).unwrap());
                            }
                            Err(TaskError::NotFound(id)) => {
                                eprintln!("Error: Task with id {} not found", id);
                                std::process::exit(1);
                            }
                            Err(e) => {
                                eprintln!("Error: {}", e);
                                std::process::exit(1);
                            }
                        }
                    } else {
                        eprintln!("Error: --id is required for get command");
                        std::process::exit(1);
                    }
                }
                TasksCommands::Create {
                    status,
                    note_id,
                    effort_estimate,
                    actual_effort,
                    deadline,
                    priority,
                    all_day,
                    goal_relationship,
                } => {
                    let deadline = match deadline {
                        Some(date_str) => {
                            match NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S") {
                                Ok(dt) => Some(dt),
                                Err(e) => {
                                    eprintln!("Error parsing deadline: {}", e);
                                    std::process::exit(1);
                                }
                            }
                        }
                        None => None,
                    };

                    let request = CreateTaskRequest {
                        note_id,
                        status,
                        effort_estimate: effort_estimate.map(|e| BigDecimal::from(e)),
                        actual_effort: actual_effort.map(|e| BigDecimal::from(e)),
                        deadline,
                        priority,
                        all_day,
                        goal_relationship: goal_relationship.map(|g| g.parse::<i32>().unwrap()),
                    };

                    match create_task(&url, request).await {
                        Ok(task) => {
                            println!("{}", serde_json::to_string_pretty(&task).unwrap());
                        }
                        Err(e) => {
                            eprintln!("Error creating task: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                TasksCommands::Update {
                    status,
                    note_id,
                    effort_estimate,
                    actual_effort,
                    deadline,
                    priority,
                    all_day,
                    goal_relationship,
                } => {
                    if let Some(task_id) = id {
                        let deadline = match deadline {
                            Some(date_str) => {
                                match NaiveDateTime::parse_from_str(&date_str, "%Y-%m-%dT%H:%M:%S")
                                {
                                    Ok(dt) => Some(dt),
                                    Err(e) => {
                                        eprintln!("Error parsing deadline: {}", e);
                                        std::process::exit(1);
                                    }
                                }
                            }
                            None => None,
                        };

                        let request = UpdateTaskRequest {
                            note_id,
                            status,
                            effort_estimate: effort_estimate.map(|e| BigDecimal::from(e)),
                            actual_effort: actual_effort.map(|e| BigDecimal::from(e)),
                            deadline,
                            priority,
                            all_day,
                            goal_relationship: goal_relationship.map(|g| g.parse::<i32>().unwrap()),
                        };

                        match update_task(&url, task_id, request).await {
                            Ok(task) => {
                                println!("{}", serde_json::to_string_pretty(&task).unwrap());
                            }
                            Err(TaskError::NotFound(id)) => {
                                eprintln!("Error: Task with id {} not found", id);
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
                TasksCommands::Delete => {
                    if let Some(task_id) = id {
                        match delete_task(&url, task_id).await {
                            Ok(_) => {
                                println!("Task {} deleted successfully", task_id);
                            }
                            Err(TaskError::NotFound(id)) => {
                                eprintln!("Error: Task with id {} not found", id);
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
                TasksCommands::Tree { simple } => match fetch_task_tree(&url).await {
                    Ok(tree) => {
                        if simple {
                            print_task_tree(&tree, 0);
                        } else {
                            println!("{}", serde_json::to_string_pretty(&tree).unwrap());
                        }
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                TasksCommands::Mappings => match fetch_hierarchy_mappings(&url).await {
                    Ok(mappings) => {
                        println!("{}", serde_json::to_string_pretty(&mappings).unwrap());
                    }
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        std::process::exit(1);
                    }
                },
                TasksCommands::Attach { parent_id } => {
                    if let Some(child_id) = id {
                        let request = AttachChildRequest {
                            parent_task_id: Some(parent_id),
                            child_task_id: child_id,
                        };
                        match attach_child_task(&url, request).await {
                            Ok(_) => {
                                println!(
                                    "Successfully attached task {} to parent {}",
                                    child_id, parent_id
                                );
                            }
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
                TasksCommands::Detach => {
                    if let Some(child_id) = id {
                        match detach_child_task(&url, child_id).await {
                            Ok(_) => {
                                println!("Successfully detached task {}", child_id);
                            }
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
            },
        },
    }
}

fn print_simple_tree(nodes: &[NoteTreeNode], depth: usize) {
    for node in nodes {
        println!(
            "{}- Note ID: {}, Title: {}",
            "  ".repeat(depth),
            node.id,
            node.title.as_deref().unwrap_or("Untitled")
        );
        if !node.children.is_empty() {
            print_simple_tree(&node.children, depth + 1);
        }
    }
}

fn print_task_tree(nodes: &[TaskTreeNode], depth: usize) {
    for node in nodes {
        print!("{}", "  ".repeat(depth));
        println!("Task ID: {}, Status: {}", node.id, node.status);

        if let Some(note_id) = node.note_id {
            print!("{}", "  ".repeat(depth + 1));
            println!("Note ID: {}", note_id);
        }

        if let Some(priority) = node.priority {
            print!("{}", "  ".repeat(depth + 1));
            println!("Priority: {}", priority);
        }

        if !node.children.is_empty() {
            print!("{}", "  ".repeat(depth + 1));
            println!("Children:");
            print_task_tree(&node.children, depth + 2);
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
