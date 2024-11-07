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
        assert "note_id" in result
        assert result["title"] == test_title
        assert result["content"] == test_content
        
    except requests.exceptions.ConnectionError:
        pytest.skip("API server is not running")
