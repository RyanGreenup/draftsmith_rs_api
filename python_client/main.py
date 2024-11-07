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

class AttachNoteRequest(BaseModel):
    child_note_id: int
    parent_note_id: int 
    hierarchy_type: str

class TreeNote(BaseModel):
    id: int
    title: str
    content: str
    created_at: Optional[datetime]
    modified_at: Optional[datetime]
    hierarchy_type: Optional[str] = None
    children: list['TreeNote']
    tags: list[str]

def update_notes_tree(notes: list[TreeNote], base_url: str = "http://localhost:37240") -> None:
    """
    Update the entire notes tree structure
    
    Args:
        notes: List of TreeNote objects representing the new tree structure
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Raises:
        requests.exceptions.RequestException: If the request fails
    """
    response = requests.put(
        f"{base_url}/notes/tree",
        headers={"Content-Type": "application/json"},
        json=[note.model_dump(exclude_unset=True) for note in notes]
    )
    
    response.raise_for_status()


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

def get_all_notes(base_url: str = "http://localhost:37240") -> list[Note]:
    """
    Retrieve all notes
    
    Args:
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Returns:
        list[Note]: List of all notes
        
    Raises:
        requests.exceptions.RequestException: If the request fails
    """
    response = requests.get(
        f"{base_url}/notes/flat",
        headers={"Content-Type": "application/json"},
    )
    
    response.raise_for_status()
    return [Note.model_validate(note) for note in response.json()]

def get_all_notes_without_content(base_url: str = "http://localhost:37240") -> list[NoteWithoutContent]:
    """
    Retrieve all notes without their content
    
    Args:
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Returns:
        list[NoteWithoutContent]: List of all notes without content
        
    Raises:
        requests.exceptions.RequestException: If the request fails
    """
    response = requests.get(
        f"{base_url}/notes/flat",
        params={"exclude_content": "true"},
        headers={"Content-Type": "application/json"},
    )
    
    response.raise_for_status()
    return [NoteWithoutContent.model_validate(note) for note in response.json()]

def attach_note_to_parent(
    child_note_id: int,
    parent_note_id: int,
    hierarchy_type: str = "block",
    base_url: str = "http://localhost:37240"
) -> None:
    """
    Attach a note as a child of another note
    
    Args:
        child_note_id: ID of the note to attach as child
        parent_note_id: ID of the parent note
        hierarchy_type: Type of hierarchy relationship (default: "block")
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Raises:
        requests.exceptions.RequestException: If the request fails
    """
    request_data = AttachNoteRequest(
        child_note_id=child_note_id,
        parent_note_id=parent_note_id,
        hierarchy_type=hierarchy_type
    )

    response = requests.post(
        f"{base_url}/notes/hierarchy/attach",
        headers={"Content-Type": "application/json"},
        data=request_data.model_dump_json(),
    )
    
    response.raise_for_status()

def get_notes_tree(base_url: str = "http://localhost:37240") -> list[TreeNote]:
    """
    Retrieve all notes in a tree structure
    
    Args:
        base_url: The base URL of the API (default: http://localhost:37240)
        
    Returns:
        list[TreeNote]: List of all notes with their hierarchical structure
        
    Raises:
        requests.exceptions.RequestException: If the request fails
    """
    response = requests.get(
        f"{base_url}/notes/tree",
        headers={"Content-Type": "application/json"},
    )
    
    response.raise_for_status()
    return [TreeNote.model_validate(note) for note in response.json()]

