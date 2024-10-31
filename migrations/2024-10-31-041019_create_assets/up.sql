-- Table for assets
CREATE TABLE assets (
 id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
 note_id INT REFERENCES notes(id),
 asset_type TEXT NOT NULL,
 location TEXT NOT NULL UNIQUE,
 description TEXT,
 description_tsv tsvector,
 created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Create an index on the tsvector column
CREATE INDEX assets_description_tsv_idx ON assets USING gin(description_tsv);

-- Trigger to update the full-text search vector
CREATE TRIGGER assets_fts_update
BEFORE INSERT OR UPDATE ON assets
FOR EACH ROW EXECUTE PROCEDURE tsvector_update_trigger(
    description_tsv, 'pg_catalog.english', description
);

