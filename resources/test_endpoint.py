import requests
import json
from dataclasses import dataclass
from typing import Optional, Dict, Any, List
from datetime import datetime

@dataclass
class EndpointMethod:
    """Represents a single HTTP method for an endpoint"""
    method: str
    description: str
    input_schema: Optional[Dict[str, Any]] = None
    output_schema: Optional[Dict[str, Any]] = None
    example_input: Optional[Dict[str, Any]] = None
    example_output: Optional[Dict[str, Any]] = None

class Endpoint:
    """Base class for API endpoints"""
    def __init__(self, base_url: str, path: str, description: str):
        self.base_url = base_url
        self.path = path
        self.description = description
        self.methods: Dict[str, EndpointMethod] = {}

    def add_method(self, method: EndpointMethod):
        self.methods[method.method] = method

    def test_endpoint(self) -> Dict[str, bool]:
        """Test all methods of this endpoint"""
        results = {}
        for method_name, method in self.methods.items():
            try:
                url = f"{self.base_url}{self.path}"
                
                if method.example_input:
                    response = requests.request(
                        method_name.upper(),
                        url,
                        json=method.example_input
                    )
                else:
                    response = requests.request(method_name.upper(), url)
                
                results[method_name] = 200 <= response.status_code < 300
                print(f"Testing {method_name} {self.path}: {response.status_code}")
                print(f"Response: {response.text}\n")
            except Exception as e:
                print(f"Error testing {method_name} {self.path}: {str(e)}")
                results[method_name] = False
        
        return results

class NotesEndpoint(Endpoint):
    """Notes API endpoint documentation and testing"""
    def __init__(self, base_url: str):
        super().__init__(
            base_url=base_url,
            path="/notes",
            description="Manages notes in the system"
        )

        # GET /notes
        self.add_method(EndpointMethod(
            method="get",
            description="List all notes",
            output_schema={
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"},
                        "title": {"type": "string"},
                        "content": {"type": "string"},
                        "created_at": {"type": "string", "format": "date-time"},
                        "modified_at": {"type": "string", "format": "date-time"}
                    }
                }
            }
        ))

        # POST /notes
        self.add_method(EndpointMethod(
            method="post",
            description="Create a new note",
            input_schema={
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["title", "content"]
            },
            example_input={
                "title": "Test Note",
                "content": "This is a test note"
            }
        ))

        # GET /notes/{id}
        self.add_method(EndpointMethod(
            method="get",
            description="Get a specific note by ID",
            path_params={"id": "integer"}
        ))

        # PUT /notes/{id}
        self.add_method(EndpointMethod(
            method="put",
            description="Update a specific note",
            path_params={"id": "integer"},
            input_schema={
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "content": {"type": "string"}
                }
            },
            example_input={
                "title": "Updated Test Note",
                "content": "This note has been updated"
            }
        ))

        # DELETE /notes/{id}
        self.add_method(EndpointMethod(
            method="delete",
            description="Delete a specific note",
            path_params={"id": "integer"}
        ))

def main():
    base_url = "http://localhost:3000"
    
    # Test Notes endpoint
    notes_endpoint = NotesEndpoint(base_url)
    results = notes_endpoint.test_endpoint()
    
    print("\nTest Results:")
    for method, success in results.items():
        print(f"{method.upper()} /notes: {'✓' if success else '✗'}")

if __name__ == "__main__":
    main()
