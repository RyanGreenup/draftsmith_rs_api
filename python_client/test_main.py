import pytest
import requests
from main import create_note

def test_create_note():
    """Test creating a note through the API endpoint"""
    # Test data
    test_title = "Test Note"
    test_content = "This is a test note"

    try:
        # Attempt to create a note
        result = create_note(test_title, test_content)

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

def test_get_note_without_content():
    """Test getting a note without content"""
    try:
        # First create a note
        note = create_note("Test Note", "Test Content")
        note_id = note["id"]
        
        # Then get it without content
        result = get_note_without_content(note_id)
        
        # Verify the response structure
        assert isinstance(result, dict)
        assert "id" in result
        assert "title" in result
        assert "created_at" in result
        assert "modified_at" in result
        
        # Content should not be included
        assert "content" not in result
        
        # Verify the id matches
        assert result["id"] == note_id
        
    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to get note: {str(e)}")
