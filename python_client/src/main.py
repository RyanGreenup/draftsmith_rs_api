from typing import Optional
from pydantic import BaseModel
import requests
import json

class CreateNoteRequest(BaseModel):
    title: str
    content: str

def create_note(title: str, content: str, base_url: str = "http://localhost:37240") -> dict:
    """
    Create a new note using the API
    
    Args:
        title: The title of the note
        content: The content of the note
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Returns:
        dict: The created note data
        
    Raises:
        requests.exceptions.RequestException: If the request fails
    """
    request_data = CreateNoteRequest(title=title, content=content)
    
    response = requests.post(
        f"{base_url}/notes/flat",
        headers={"Content-Type": "application/json"},
        data=request_data.model_dump_json()
    )
    
    response.raise_for_status()
    return response.json()