# Draftsmith Python Client

Official Python bindings for Draftsmith's REST API.

## What is Draftsmith?

<p><img src="./assets/logo.png" style="float: left; width: 80px" /></p>

Draftsmith is a modern note-taking and task management system built with a focus on performance, type safety, and flexibility. It allows you to organize your thoughts, tasks, and knowledge in a hierarchical structure while maintaining relationships between different pieces of information through tags and parent-child relationships.

## Architecture

Draftsmith follows a unique architectural approach:
- Core logic is implemented in Rust (backed by Postgresql), providing strong type safety, memory safety, and high performance
- A REST API exposes this functionality, allowing for flexible integrations
- Client applications (GUI, CLI, etc.) can be written in any language while benefiting from a stable, well-defined and performant backend

This separation allows:
- Core business logic to remain fast and reliable
- GUI development to use more flexible languages like Python or TypeScript
- CLI tools and aliases can be created easily.
- Type safety and correctness to be maintained across all interfaces

## Installation

```bash
pip install draftsmith-client
```

## Usage

```python
from draftsmith import note_create, get_notes_tree, create_task, TaskStatus

# Create a new note
note = note_create("Meeting Notes", "Discussion points for today's meeting")

# Create a task
task = create_task(CreateTaskRequest(
    status=TaskStatus.TODO,
    priority=1,
    all_day=False
))

# Get hierarchical view of all notes
notes_tree = get_notes_tree()
```

## Features

- Complete API coverage for Draftsmith's functionality
    - Full CRUD operations for notes, tasks, and tags
- Type-safe requests and responses using Pydantic models

## API Documentation

The client provides bindings for all Draftsmith REST API endpoints:

### Notes
- Create, read, update, and delete notes
- Manage note hierarchies
- Retrieve notes with or without content
- Get full note trees

### Tasks
- Create and manage tasks
- Set status, priority, and deadlines
- Organize tasks in hierarchies
- Track effort estimates and actual effort

### Tags
- Create and manage tags
- Attach tags to notes
- Organize tags in hierarchies
- Get tagged items in tree structure

## Development

To run tests:

```bash
pytest python_client/test_main.py
```

## License

GPL License
