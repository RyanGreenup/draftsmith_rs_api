from typing import Optional
from datetime import datetime
from pydantic import BaseModel
import requests
import json


class CreateNoteRequest(BaseModel):
    title: str
    content: str

class Note(BaseModel):
    id: int
    title: str
    content: str 
    created_at: datetime
    modified_at: datetime

class NoteWithoutContent(BaseModel):
    id: int
    title: str
    created_at: datetime
    modified_at: datetime


def note_create(
    title: str, content: str, base_url: str = "http://localhost:37240"
) -> dict:
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
        data=request_data.model_dump_json(),
    )

    response.raise_for_status()
    return response.json()

def get_note(note_id: int, base_url: str = "http://localhost:37240") -> Note:
    """
    Retrieve a note by its ID
    
    Args:
        note_id: The ID of the note to retrieve
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Returns:
        Note: The retrieved note data
        
    Raises:
        requests.exceptions.RequestException: If the request fails
        requests.exceptions.HTTPError: If the note is not found (404)
    """
    response = requests.get(
        f"{base_url}/notes/flat/{note_id}",
        headers={"Content-Type": "application/json"},
    )
    
    response.raise_for_status()
    return Note.model_validate(response.json())

def get_note_without_content(note_id: int, base_url: str = "http://localhost:37240") -> NoteWithoutContent:
    """
    Retrieve a note by its ID, excluding the content field
    
    Args:
        note_id: The ID of the note to retrieve
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Returns:
        NoteWithoutContent: The retrieved note data without content
        
    Raises:
        requests.exceptions.RequestException: If the request fails
        requests.exceptions.HTTPError: If the note is not found (404)
    """
    response = requests.get(
        f"{base_url}/notes/flat/{note_id}",
        params={"exclude_content": "true"},
        headers={"Content-Type": "application/json"},
    )
    
    response.raise_for_status()
    return NoteWithoutContent.model_validate(response.json())

