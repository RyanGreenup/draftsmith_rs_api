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

