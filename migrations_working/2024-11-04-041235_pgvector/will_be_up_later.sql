-- Enable pgvector extension
CREATE EXTENSION IF NOT EXISTS vector;

-- Create note_embeddings table
CREATE TABLE note_embeddings (
    note_id INT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    embedding vector(1024) NOT NULL
);

-- Create an index for similarity search
CREATE INDEX note_embeddings_vector_idx ON note_embeddings
USING ivfflat (embedding vector_l2_ops)
WITH (lists = 100);
