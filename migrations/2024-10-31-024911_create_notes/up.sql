-- * Notes and Tags -----------------------------------------------------------
-- ** Notes -------------------------------------------------------------------
-- *** Main Table -------------------------------------------------------------
-- Table to store notes with a full-text search
CREATE TABLE notes (
--    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
-- Use serial so ID's are sequential when there are deletions
-- more convenient for the user
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    fts TSVECTOR
);

-- *** Index ------------------------------------------------------------------
CREATE INDEX notes_fts_idx ON notes USING gin (fts);

-- *** FTS Trigger ------------------------------------------------------------
-- Trigger to update the full-text search vector
CREATE TRIGGER notes_fts_update
BEFORE INSERT OR UPDATE ON notes
FOR EACH ROW EXECUTE PROCEDURE TSVECTOR_UPDATE_TRIGGER(
    fts, 'pg_catalog.english', title, content
);

-- *** Auto update modified_at ------------------------------------------------
CREATE OR REPLACE FUNCTION update_modified_at_column_on_notes()
RETURNS TRIGGER AS $$
BEGIN
    NEW.modified_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_modified_at
BEFORE UPDATE ON notes
FOR EACH ROW
EXECUTE FUNCTION update_modified_at_column_on_notes();

-- *** Hierarchy --------------------------------------------------------------
CREATE TABLE note_hierarchy (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    parent_note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    child_note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    hierarchy_type TEXT CHECK (hierarchy_type IN ('page', 'block', 'subpage')),
    -- This enforces that each child note can only have one parent
    UNIQUE (child_note_id)
);

-- *** Track Modification Dates -----------------------------------------------
-- **** Modification Table ----------------------------------------------------

CREATE TABLE note_modifications (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    previous_content TEXT NOT NULL,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- **** Trigger ---------------------------------------------------------------
-- ***** Function -------------------------------------------------------------
CREATE OR REPLACE FUNCTION LOG_NOTE_MODIFICATIONS()
RETURNS TRIGGER
LANGUAGE plpgsql AS
$func$
BEGIN
   IF TG_OP = 'UPDATE' THEN
       INSERT INTO note_modifications (note_id, previous_content, modified_at)
       VALUES (OLD.id, OLD.content, CURRENT_TIMESTAMP);
   END IF;
   RETURN NEW;
END
$func$;

-- ***** Attach Trigger -------------------------------------------------------
CREATE TRIGGER track_note_modifications
BEFORE UPDATE ON notes
FOR EACH ROW EXECUTE FUNCTION LOG_NOTE_MODIFICATIONS();

-- ** Tags --------------------------------------------------------------------
-- Table for tags
CREATE TABLE tags (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT NOT NULL
);

CREATE TABLE note_tags (
    note_id INT NOT NULL REFERENCES notes (id) ON DELETE CASCADE,
    tag_id INT NOT NULL REFERENCES tags (id),
    PRIMARY KEY (note_id, tag_id)
);

CREATE TABLE tag_hierarchy (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    parent_tag_id INT REFERENCES tags (id),
    child_tag_id INT REFERENCES tags (id),
    UNIQUE (child_tag_id)  -- Tags can only have one parent
);


-- ** Note Attributes----------------------------------------------------------
-- Table for misc attributes
CREATE TABLE attributes (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE note_attributes (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    attribute_id INT REFERENCES attributes (id),
    value TEXT NOT NULL
);

-- Populate initial data for attributes
INSERT INTO attributes (name, description) VALUES
('location', 'Location of the note'),
('author', 'Author of the note'),
('source', 'Source of the note');


-- ** Note Types --------------------------------------------------------------

-- Table for note types
CREATE TABLE note_types (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE note_type_mappings (
    note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    type_id INT REFERENCES note_types (id),
    PRIMARY KEY (note_id, type_id)
);


-- Populate initial data for note types
INSERT INTO note_types (name, description) VALUES
('asset', 'Asset related notes'),
('bookmark', 'Bookmark related notes'),
('contact', 'Contact information'),
('page', 'A standalone page'),
('block', 'A block of information within a page'),
('subpage', 'A subpage within a note');

-- ** Journal Entries ---------------------------------------------------------
-- Table for journal/calendar view (unused)
CREATE TABLE journal_entries (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    entry_date DATE NOT NULL
);
