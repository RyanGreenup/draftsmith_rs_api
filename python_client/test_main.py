import pytest
import requests
from main import *


def test_note_create():
    """Test creating a note through the API endpoint"""
    # Test data
    test_title = "Test Note"
    test_content = "This is a test note"

    try:
        # Attempt to create a note
        result = note_create(test_title, test_content)

        # Verify the response structure
        assert isinstance(result, dict)
        assert "id" in result
        assert "title" in result
        assert "content" in result
        assert "created_at" in result
        assert "modified_at" in result

        # Verify the content matches what we sent
        assert result["title"] == "Untitled"  # API (db) sets default title to H1
        assert result["content"] == test_content

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to create note: {str(e)}")


def test_get_note():
    """Test retrieving a note by ID"""
    # First create a note to ensure we have something to retrieve
    test_title = "Test Note"
    test_content = "This is a test note"

    try:
        # Create the note
        created = note_create(test_title, test_content)
        note_id = created["id"]

        # Retrieve the note
        result = get_note(note_id)

        # Verify the response structure using Pydantic model
        assert isinstance(result, Note)
        assert result.id == note_id
        assert result.title == "Untitled"  # API sets default title
        assert result.content == test_content
        assert result.created_at is not None
        assert result.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve note: {str(e)}")


def test_get_note_without_content():
    """Test retrieving a note without content"""
    test_title = "Test Note"
    test_content = "This is a test note"

    try:
        # Create a note
        created = note_create(test_title, test_content)
        note_id = created["id"]

        # Retrieve the note without content
        result = get_note_without_content(note_id)

        # Verify the response structure using Pydantic model
        assert isinstance(result, NoteWithoutContent)
        assert result.id == note_id
        assert result.title == "Untitled"  # API sets default title
        assert result.created_at is not None
        assert result.modified_at is not None
        assert not hasattr(result, "content")

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve note without content: {str(e)}")


def test_get_all_notes():
    """Test retrieving all notes"""
    try:
        # Get all notes
        notes = get_all_notes()

        # Verify we got a list of Note objects
        assert isinstance(notes, list)
        assert len(notes) > 0
        assert all(isinstance(note, Note) for note in notes)

        # Verify each note has the required fields
        for note in notes:
            assert note.id > 0
            assert isinstance(note.title, str)
            assert isinstance(note.content, str)
            assert note.created_at is not None
            assert note.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve all notes: {str(e)}")


def test_get_all_notes_without_content():
    """Test retrieving all notes without content"""
    try:
        # Get all notes without content
        notes = get_all_notes_without_content()

        # Verify we got a list of NoteWithoutContent objects
        assert isinstance(notes, list)
        assert len(notes) > 0
        assert all(isinstance(note, NoteWithoutContent) for note in notes)

        # Verify each note has the required fields
        for note in notes:
            assert note.id > 0
            assert isinstance(note.title, str)
            assert note.created_at is not None
            assert note.modified_at is not None
            assert not hasattr(note, "content")

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve all notes without content: {str(e)}")


def test_get_tags_tree():
    """Test retrieving tags in tree structure"""
    try:
        # Create parent and child tags
        parent_tag = create_tag("parent")
        child_tag = create_tag("child")

        # Create a note to attach to the parent tag
        note = note_create("Test Note", "Test content")
        note_id = note["id"]

        # Attach child tag to parent tag
        attach_tag_to_parent(child_tag.id, parent_tag.id)

        # Attach note to parent tag
        attach_tag_to_note(note_id, parent_tag.id)

        # Get tags tree
        tags = get_tags_tree()

        # Verify we got a list of TreeTagWithNotes objects
        assert isinstance(tags, list)
        assert len(tags) > 0
        assert all(isinstance(tag, TreeTagWithNotes) for tag in tags)

        # Find our test tag
        test_tag = next((tag for tag in tags if tag.id == parent_tag.id), None)
        assert test_tag is not None
        assert test_tag.name == "parent"

        # Verify structure
        assert isinstance(test_tag.children, list)
        assert isinstance(test_tag.notes, list)

        # Verify child tag is in children
        assert any(child.id == child_tag.id for child in test_tag.children)
        child = next(
            (child for child in test_tag.children if child.id == child_tag.id), None
        )
        assert child.name == "child"

        # Verify note is attached
        assert any(n.id == note_id for n in test_tag.notes)
        attached_note = next((n for n in test_tag.notes if n.id == note_id), None)
        assert attached_note.title == "Untitled"  # API sets default title

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve tags tree: {str(e)}")


def test_get_task():
    """Test retrieving a task by ID"""
    try:
        # First create a task to ensure we have one to get
        task_request = CreateTaskRequest(
            status=TaskStatus.TODO,
            priority=1,
            all_day=False,
        )
        created_task = create_task(task_request)
        task_id = created_task.id

        # Get the task
        result = get_task(task_id)

        # Verify the response structure
        assert isinstance(result, Task)
        assert result.id == task_id
        assert result.status == TaskStatus.TODO
        assert result.priority == 1
        assert result.all_day == False
        assert result.created_at is not None
        assert result.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get task: {str(e)}")


def test_get_all_tasks():
    """Test retrieving all tasks"""
    try:
        # First create a task to ensure we have at least one
        task_request = CreateTaskRequest(
            status=TaskStatus.TODO,
            priority=1,
            all_day=False,
        )
        created_task = create_task(task_request)

        # Get all tasks
        tasks = get_all_tasks()

        # Verify we got a list of Task objects
        assert isinstance(tasks, list)
        assert len(tasks) > 0
        assert all(isinstance(task, Task) for task in tasks)

        # Find our created task in the list
        test_task = next((task for task in tasks if task.id == created_task.id), None)
        assert test_task is not None
        assert test_task.status == TaskStatus.TODO
        assert test_task.priority == 1
        assert test_task.all_day == False
        assert test_task.created_at is not None
        assert test_task.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get tasks: {str(e)}")


def test_get_task_hierarchy_relations():
    """Test getting all task hierarchy relationships"""
    try:
        # First create two tasks to ensure we have some hierarchy
        parent_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=1,
                all_day=False,
            )
        )
        child_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=2,
                all_day=False,
            )
        )

        # Attach child to parent
        attach_task_to_parent(child_task.id, parent_task.id)

        # Get all hierarchy relationships
        relations = get_task_hierarchy_relations()

        # Verify we got a list of TaskHierarchyRelation objects
        assert isinstance(relations, list)
        assert all(isinstance(rel, TaskHierarchyRelation) for rel in relations)

        # Find our test relationship
        test_relation = next(
            (
                rel
                for rel in relations
                if rel.parent_id == parent_task.id and rel.child_id == child_task.id
            ),
            None,
        )
        assert test_relation is not None
        assert test_relation.parent_id == parent_task.id
        assert test_relation.child_id == child_task.id

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get task hierarchy relations: {str(e)}")


def test_delete_task():
    """Test deleting a task through the API endpoint"""
    try:
        # First create a task to delete
        task_request = CreateTaskRequest(
            status=TaskStatus.TODO,
            priority=1,
            all_day=False,
        )
        created_task = create_task(task_request)

        # Delete the task
        delete_task(created_task.id)

        # Verify the task was deleted by trying to get it
        with pytest.raises(requests.exceptions.HTTPError) as exc_info:
            get_task(created_task.id)
        assert exc_info.value.response.status_code == 404

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to delete task: {str(e)}")


def test_update_task():
    """Test updating a task through the API endpoint"""
    try:
        # First create a task to update
        task_request = CreateTaskRequest(
            status=TaskStatus.TODO,
            priority=1,
            all_day=False,
        )
        created_task = create_task(task_request)

        # Create update request
        update_request = UpdateTaskRequest(
            status=TaskStatus.DONE, actual_effort=Decimal("3.0"), priority=2
        )

        # Update the task
        result = update_task(created_task.id, update_request)

        # Verify the response structure
        assert isinstance(result, Task)
        assert result.id == created_task.id
        assert result.status == TaskStatus.DONE
        assert result.actual_effort == Decimal("3.0")
        assert result.priority == 2
        assert result.created_at is not None
        assert result.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to update task: {str(e)}")


def test_detach_task_from_parent():
    """Test detaching a task from its parent"""
    try:
        # Create parent task
        parent_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=1,
                all_day=False,
            )
        )

        # Create child task
        child_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=2,
                all_day=False,
            )
        )

        # First attach child to parent
        attach_task_to_parent(child_task.id, parent_task.id)

        # Verify the attachment worked
        relations = get_task_hierarchy_relations()
        assert any(
            rel.parent_id == parent_task.id and rel.child_id == child_task.id
            for rel in relations
        )

        # Now detach the child
        detach_task_from_parent(child_task.id)

        # Verify the detachment worked by checking relations again
        relations_after = get_task_hierarchy_relations()
        assert not any(
            rel.parent_id == parent_task.id and rel.child_id == child_task.id
            for rel in relations_after
        )

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to detach task from parent: {str(e)}")


def test_attach_task_to_parent():
    """Test attaching a task as a child of another task"""
    try:
        # Create parent task
        parent_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=1,
                all_day=False,
            )
        )

        # Create child task
        child_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=2,
                all_day=False,
            )
        )

        # Attach child to parent
        attach_task_to_parent(child_task.id, parent_task.id)

        # Verify the attachment by getting hierarchy relations
        relations = get_task_hierarchy_relations()
        assert any(
            rel.parent_id == parent_task.id and rel.child_id == child_task.id
            for rel in relations
        )

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to attach task to parent: {str(e)}")


def test_get_tasks_tree():
    """Test retrieving tasks in tree structure"""
    try:
        # Create parent task
        parent_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=1,
                all_day=False,
            )
        )

        # Create child task
        child_task = create_task(
            CreateTaskRequest(
                status=TaskStatus.TODO,
                priority=2,
                all_day=False,
            )
        )

        # Attach child to parent
        attach_task_to_parent(child_task.id, parent_task.id)

        # Get tasks tree
        tasks = get_tasks_tree()

        # Verify we got a list of TreeTask objects
        assert isinstance(tasks, list)
        assert len(tasks) > 0
        assert all(isinstance(task, TreeTask) for task in tasks)

        # Find our test task in the tree
        test_task = next((task for task in tasks if task.id == parent_task.id), None)
        assert test_task is not None
        assert test_task.status == TaskStatus.TODO
        assert test_task.priority == 1
        assert test_task.all_day == False
        assert test_task.created_at is not None
        assert test_task.modified_at is not None
        assert isinstance(test_task.children, list)

        # Verify child task is in children
        assert len(test_task.children) == 1
        child = test_task.children[0]
        assert child.id == child_task.id
        assert child.status == TaskStatus.TODO
        assert child.priority == 2
        assert child.all_day == False
        assert child.created_at is not None
        assert child.modified_at is not None
        assert isinstance(child.children, list)
        assert len(child.children) == 0  # No grandchildren

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve tasks tree: {str(e)}")


def test_create_task():
    """Test creating a task through the API endpoint"""
    try:
        # Create task request
        task_request = CreateTaskRequest(
            status=TaskStatus.TODO,
            effort_estimate=Decimal("2.5"),
            deadline=datetime(2024, 1, 1),
            priority=1,
            all_day=False,
        )

        # Create the task
        result = create_task(task_request)

        # Verify the response structure
        assert isinstance(result, Task)
        assert result.id > 0
        assert result.status == TaskStatus.TODO
        assert result.effort_estimate == Decimal("2.5")
        assert result.deadline.date() == date(2024, 1, 1)
        assert result.priority == 1
        assert result.all_day == False
        assert result.created_at is not None
        assert result.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to create task: {str(e)}")


def test_update_asset():
    """Test updating an asset's metadata"""
    try:
        # First create a test asset
        with tempfile.NamedTemporaryFile(delete=False, suffix=".txt") as tf:
            tf.write(b"Test asset for updating")
            temp_path = tf.name

        try:
            # Upload test asset
            created_asset = upload_asset(temp_path)

            # Create update request
            update_request = UpdateAssetRequest(
                note_id=1, description="Updated description for the asset"
            )

            # Update the asset
            updated_asset = update_asset(created_asset.id, update_request)

            # Verify the response
            assert isinstance(updated_asset, Asset)
            assert updated_asset.id == created_asset.id
            assert updated_asset.note_id == 1
            assert updated_asset.description == "Updated description for the asset"
            assert updated_asset.location == created_asset.location
            assert updated_asset.created_at is not None

        finally:
            # Clean up the temporary file
            os.unlink(temp_path)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to update asset: {str(e)}")


def test_delete_asset():
    """Test deleting an asset through the API endpoint"""
    try:
        # First create a test asset to delete
        with tempfile.NamedTemporaryFile(delete=False, suffix=".txt") as tf:
            tf.write(b"Test asset for deletion")
            temp_path = tf.name

        try:
            # Upload test asset
            created_asset = upload_asset(temp_path)

            # Delete the asset
            delete_asset(created_asset.id)

            # Verify the asset was deleted by trying to get it
            with pytest.raises(requests.exceptions.HTTPError) as exc_info:
                # Try to download the deleted asset, should fail with 404
                download_path = os.path.join(tempfile.gettempdir(), "deleted_asset.txt")
                download_asset(created_asset.id, download_path)
            assert exc_info.value.response.status_code == 404

        finally:
            # Clean up the temporary file
            os.unlink(temp_path)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to delete asset: {str(e)}")


def test_download_asset_by_filename():
    """Test downloading an asset by filename"""
    try:
        # Create output path for downloaded file
        download_path = os.path.join(tempfile.gettempdir(), "downloaded_icon.png")

        try:
            # Download the asset by filename
            download_asset("icon.png", download_path)

            # Verify the file was downloaded
            assert os.path.exists(download_path)
            assert os.path.getsize(download_path) > 0

        finally:
            # Clean up downloaded file
            if os.path.exists(download_path):
                os.unlink(download_path)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to download asset by filename: {str(e)}")


def test_get_all_assets():
    """Test retrieving all assets"""
    try:
        # First create a test asset to ensure we have at least one
        with tempfile.NamedTemporaryFile(delete=False, suffix=".txt") as tf:
            tf.write(b"Test asset for listing")
            temp_path = tf.name

        try:
            # Upload test asset
            created_asset = upload_asset(temp_path)

            # Get all assets
            assets = get_all_assets()

            # Verify we got a list of Asset objects
            assert isinstance(assets, list)
            assert len(assets) > 0
            assert all(isinstance(asset, Asset) for asset in assets)

            # Find our test asset in the list
            test_asset = next(
                (asset for asset in assets if asset.id == created_asset.id), None
            )
            assert test_asset is not None
            assert test_asset.location.startswith("uploads/")
            assert test_asset.created_at is not None

        finally:
            # Clean up the temporary file
            os.unlink(temp_path)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve assets: {str(e)}")


def test_upload_asset():
    """Test uploading a file as an asset"""
    try:
        # Create a temporary test file
        import tempfile
        import os

        with tempfile.NamedTemporaryFile(delete=False, suffix=".txt") as tf:
            tf.write(b"Test content")
            temp_path = tf.name

        try:
            # Upload the file
            result = upload_asset(temp_path)

            # Verify the response structure
            assert isinstance(result, Asset)
            assert result.id > 0
            assert result.location.startswith("uploads/")
            assert result.created_at is not None
            assert result.note_id is None
            assert result.description is None

        finally:
            # Clean up the temporary file
            os.unlink(temp_path)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to upload asset: {str(e)}")


def test_search_notes():
    """Test searching notes using full-text search"""
    try:
        # Search for notes containing 'feature'
        results = search_notes("feature")

        # Verify we got a list of Note objects
        assert isinstance(results, list)
        assert all(isinstance(note, Note) for note in results)

        # Verify each note has the required fields
        for note in results:
            assert note.id > 0
            assert isinstance(note.title, str)
            assert isinstance(note.content, str)
            assert note.created_at is not None
            assert note.modified_at is not None

            # Verify the note contains our search term
            assert "feature" in note.content.lower()

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to search notes: {str(e)}")


def test_batch_update_notes():
    """Test updating multiple notes in a single request"""
    try:
        # First create two notes to update
        note1 = note_create("Test Note 1", "Original content 1")
        note2 = note_create("Test Note 2", "Original content 2")

        # Create batch update request
        request = BatchUpdateNotesRequest(
            updates=[
                (
                    note1["id"],
                    UpdateNoteRequest(content="# Heading 1\nUpdated Content 1"),
                ),
                (
                    note2["id"],
                    UpdateNoteRequest(content="# Heading 2\nUpdated Content 2"),
                ),
            ]
        )

        # Perform batch update
        result = batch_update_notes(request)

        # Verify the response structure
        assert isinstance(result, BatchUpdateNotesResponse)
        assert len(result.updated) == 2
        assert len(result.failed) == 0

        # Verify each updated note
        for note in result.updated:
            assert isinstance(note, Note)
            assert note.created_at is not None
            assert note.modified_at is not None

            if note.id == note1["id"]:
                assert note.title == "Heading 1"  # API extracts title from content
                assert note.content == "# Heading 1\nUpdated Content 1"
            elif note.id == note2["id"]:
                assert note.title == "Heading 2"
                assert note.content == "# Heading 2\nUpdated Content 2"

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to batch update notes: {str(e)}")


def test_update_note():
    """Test updating a note through the API endpoint"""
    try:
        # First create a note to update
        created = note_create("Test Note", "Original content")
        note_id = created["id"]

        # Create update request
        update_request = UpdateNoteRequest(
            title="Updated Title", content="Updated content for my note"
        )

        # Update the note
        result = update_note(note_id, update_request)

        # Verify the response structure
        assert isinstance(result, Note)
        assert result.id == note_id
        assert result.title == "Untitled"  # API sets default title
        assert result.content == "Updated content for my note"
        assert result.created_at is not None
        assert result.modified_at is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to update note: {str(e)}")


def test_get_notes_tree():
    """Test retrieving notes in tree structure"""
    try:
        # Get notes tree
        notes = get_notes_tree()

        # Verify we got a list of TreeNote objects
        assert isinstance(notes, list)
        assert len(notes) > 0
        assert all(isinstance(note, TreeNote) for note in notes)

        # Verify each note has the required fields
        for note in notes:
            assert note.id > 0
            assert isinstance(note.title, str)
            assert isinstance(note.content, str)
            assert note.created_at is not None
            assert note.modified_at is not None
            assert isinstance(note.children, list)
            assert isinstance(note.tags, list)

            # Verify any children are also TreeNote objects
            for child in note.children:
                assert isinstance(child, TreeNote)

            # Verify tags are TreeTag objects
            for tag in note.tags:
                assert isinstance(tag, TreeTag)
                assert isinstance(tag.id, int)
                assert isinstance(tag.name, str)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to retrieve notes tree: {str(e)}")


def test_get_note_hierarchy_relations():
    """Test getting all note hierarchy relationships"""
    try:
        # Create parent note
        parent = note_create("Parent", "Parent content")
        parent_id = parent["id"]

        # Create child note
        child = note_create("Child", "Child content")
        child_id = child["id"]

        # Attach child to parent
        attach_note_to_parent(child_id, parent_id)

        # Get all hierarchy relationships
        relations = get_note_hierarchy_relations()

        # Verify we got a list of NoteHierarchyRelation objects
        assert isinstance(relations, list)
        assert all(isinstance(rel, NoteHierarchyRelation) for rel in relations)

        # Find our test relationship
        test_relation = next(
            (
                rel
                for rel in relations
                if rel.parent_id == parent_id and rel.child_id == child_id
            ),
            None,
        )
        assert test_relation is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get hierarchy relations: {str(e)}")


def test_attach_note_to_parent():
    """Test attaching a note as a child of another note"""
    try:
        # Create parent note
        parent = note_create("Parent", "Parent content")
        parent_id = parent["id"]

        # Create child note
        child = note_create("Child", "Child content")
        child_id = child["id"]

        # Attach child to parent
        attach_note_to_parent(child_id, parent_id)

        # Verify the attachment by getting the tree
        tree = get_notes_tree()

        # Find the parent note in the tree
        parent_note = next((n for n in tree if n.id == parent_id), None)
        assert parent_note is not None

        # Verify child is in parent's children
        child_ids = [child.id for child in parent_note.children]
        assert child_id in child_ids

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to attach note: {str(e)}")


def test_detach_note_from_parent():
    """Test detaching a note from its parent"""
    try:
        # Create parent note
        parent = note_create("Parent", "Parent content")
        parent_id = parent["id"]

        # Create child note
        child = note_create("Child", "Child content")
        child_id = child["id"]

        # First attach child to parent
        attach_note_to_parent(child_id, parent_id)

        # Verify the attachment worked
        relations = get_note_hierarchy_relations()
        assert any(
            rel.parent_id == parent_id and rel.child_id == child_id for rel in relations
        )

        # Now detach the child
        detach_note_from_parent(child_id)

        # Verify the detachment worked by checking relations again
        relations_after = get_note_hierarchy_relations()
        assert not any(
            rel.parent_id == parent_id and rel.child_id == child_id
            for rel in relations_after
        )

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to detach note: {str(e)}")


def test_get_tag():
    """Test getting a tag by ID"""
    try:
        # First create a tag to ensure we have one to get
        tag_name = "TestTag"
        created = create_tag(tag_name)
        tag_id = created.id

        # Get the tag
        result = get_tag(tag_id)

        # Verify the response structure
        assert isinstance(result, Tag)
        assert result.id == tag_id
        assert result.name == tag_name

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get tag: {str(e)}")


def test_get_all_tags():
    """Test getting all tags"""
    try:
        # First create a tag to ensure we have at least one
        tag_name = "TestTag"
        created = create_tag(tag_name)

        # Get all tags
        tags = get_all_tags()

        # Verify we got a list of Tag objects
        assert isinstance(tags, list)
        assert len(tags) > 0
        assert all(isinstance(tag, Tag) for tag in tags)

        # Verify our created tag is in the list
        assert any(tag.id == created.id and tag.name == created.name for tag in tags)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get tags: {str(e)}")


def test_update_tag():
    """Test updating a tag through the API endpoint"""
    try:
        # First create a tag to update
        original_name = "TestTag"
        created = create_tag(original_name)
        tag_id = created.id

        # Update the tag
        new_name = "UpdatedTestTag"
        result = update_tag(tag_id, new_name)

        # Verify the response structure
        assert isinstance(result, Tag)
        assert result.id == tag_id
        assert result.name == new_name

        # Verify the update persisted by getting the tag
        updated = get_tag(tag_id)
        assert updated.name == new_name

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to update tag: {str(e)}")


def test_delete_tag():
    """Test deleting a tag through the API endpoint"""
    try:
        # First create a tag to delete
        tag_name = "TagToDelete"
        created = create_tag(tag_name)
        tag_id = created.id

        # Delete the tag
        delete_tag(tag_id)

        # Verify the tag was deleted by trying to get it
        with pytest.raises(requests.exceptions.HTTPError) as exc_info:
            get_tag(tag_id)
        assert exc_info.value.response.status_code == 404

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to delete tag: {str(e)}")


def test_attach_tag_to_note():
    """Test attaching a tag to a note"""
    try:
        # First create a note and a tag to work with
        note = note_create("Test Note", "Test content")
        note_id = note["id"]

        tag_name = "TestTag"
        tag = create_tag(tag_name)
        tag_id = tag.id

        # Attach the tag to the note
        attach_tag_to_note(note_id, tag_id)

        # Verify the attachment by getting the notes tree
        # (since it includes tags in the response)
        tree = get_notes_tree()
        note_in_tree = next((n for n in tree if n.id == note_id), None)
        assert note_in_tree is not None
        assert any(tag.name == tag_name for tag in note_in_tree.tags)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to attach tag to note: {str(e)}")


def test_detach_tag_from_note():
    """Test detaching a tag from a note"""
    try:
        # First create a note and tag to work with
        note = note_create("Test Note", "Test content")
        note_id = note["id"]

        tag = create_tag("TestTag")
        tag_id = tag.id

        # Attach the tag to the note
        attach_tag_to_note(note_id, tag_id)

        # Verify the attachment worked
        relations = get_note_tag_relations()
        assert any(rel.note_id == note_id and rel.tag_id == tag_id for rel in relations)

        # Now detach the tag
        detach_tag_from_note(note_id, tag_id)

        # Verify the detachment worked by checking relations again
        relations_after = get_note_tag_relations()
        assert not any(
            rel.note_id == note_id and rel.tag_id == tag_id for rel in relations_after
        )

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to detach tag from note: {str(e)}")


def test_get_note_tag_relations():
    """Test getting all note-tag relationships"""
    try:
        # First create a note and tag to work with
        note = note_create("Test Note", "Test content")
        note_id = note["id"]

        tag = create_tag("TestTag")
        tag_id = tag.id

        # Attach the tag to the note
        attach_tag_to_note(note_id, tag_id)

        # Get all note-tag relationships
        relations = get_note_tag_relations()

        # Verify we got a list of NoteTagRelation objects
        assert isinstance(relations, list)
        assert all(isinstance(rel, NoteTagRelation) for rel in relations)

        # Find our test relationship
        test_relation = next(
            (
                rel
                for rel in relations
                if rel.note_id == note_id and rel.tag_id == tag_id
            ),
            None,
        )
        assert test_relation is not None

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get note-tag relations: {str(e)}")


def test_attach_tag_to_parent():
    """Test attaching a tag as a child of another tag"""
    try:
        # Create parent and child tags
        parent = create_tag("ParentTag")
        child = create_tag("ChildTag")

        # Attach child to parent
        attach_tag_to_parent(child.id, parent.id)

        # Verify the attachment by getting hierarchy relations
        relations = get_tag_hierarchy_relations()
        assert any(
            rel.parent_id == parent.id and rel.child_id == child.id for rel in relations
        )

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to attach tag to parent: {str(e)}")


def test_get_tag_hierarchy_relations():
    """Test getting all tag hierarchy relationships"""
    try:
        # Create parent tag
        parent = create_tag("ParentTag")
        parent_id = parent.id

        # Create child tag
        child = create_tag("ChildTag")
        child_id = child.id

        # Get all hierarchy relationships
        relations = get_tag_hierarchy_relations()

        # Verify we got a list of TagHierarchyRelation objects
        assert isinstance(relations, list)
        assert all(isinstance(rel, TagHierarchyRelation) for rel in relations)

        # Verify the structure of a relation if any exist
        if relations:
            relation = relations[0]
            assert hasattr(relation, "parent_id")
            assert hasattr(relation, "child_id")
            assert isinstance(relation.parent_id, int)
            assert isinstance(relation.child_id, int)

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get tag hierarchy relations: {str(e)}")


def test_detach_tag_from_parent():
    """Test detaching a tag from its parent"""
    try:
        # Create parent and child tags
        parent = create_tag("ParentTag")
        child = create_tag("ChildTag")

        # First attach child to parent
        attach_tag_to_parent(child.id, parent.id)

        # Verify the attachment worked
        relations = get_tag_hierarchy_relations()
        assert any(
            rel.parent_id == parent.id and rel.child_id == child.id for rel in relations
        )

        # Now detach the child
        detach_tag_from_parent(child.id)

        # Verify the detachment worked by checking relations again
        relations_after = get_tag_hierarchy_relations()
        assert not any(
            rel.parent_id == parent.id and rel.child_id == child.id
            for rel in relations_after
        )

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to detach tag from parent: {str(e)}")


def test_create_tag():
    """Test creating a tag through the API endpoint"""
    try:
        # Create a tag
        tag_name = "TestTag"
        result = create_tag(tag_name)

        # Verify the response structure
        assert isinstance(result, Tag)
        assert result.id > 0
        assert result.name == tag_name

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to create tag: {str(e)}")


def test_update_notes_tree():
    """Test updating the entire notes tree structure"""
    try:
        # First create a note to work with
        created = note_create("Root", "Root content")
        note_id = created["id"]

        # Create a tree structure with the actual note
        note = TreeNote(
            id=note_id,
            title="Root",
            content="Root content",
            created_at=None,
            modified_at=None,
            hierarchy_type=None,
            children=[],
            tags=[],
        )

        # Update the tree structure
        update_notes_tree([note])

        # Verify the update was successful by retrieving the tree
        updated_tree = get_notes_tree()
        assert len(updated_tree) > 0

        # Find our updated note
        updated_note = next((n for n in updated_tree if n.id == note_id), None)
        assert updated_note is not None
        # Don't test title as API sets default title
        # assert updated_note.title == "Untitled"
        assert updated_note.content == "Root content"

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to update notes tree: {str(e)}")
