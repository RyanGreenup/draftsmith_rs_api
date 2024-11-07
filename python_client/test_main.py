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
        assert result["title"] == test_title
        assert result["content"] == test_content

    except requests.exceptions.RequestException as e:
        pytest.fail(f"Failed to create note: {str(e)}")
