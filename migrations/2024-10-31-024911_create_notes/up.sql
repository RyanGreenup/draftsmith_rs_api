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
CREATE OR REPLACE FUNCTION UPDATE_MODIFIED_AT_COLUMN_ON_NOTES()
RETURNS TRIGGER AS $$
BEGIN
    NEW.modified_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_modified_at
BEFORE UPDATE ON notes
FOR EACH ROW
EXECUTE FUNCTION UPDATE_MODIFIED_AT_COLUMN_ON_NOTES();

-- **** Title as H1 -----------------------------------------------------------
-- ***** Function -------------------------------------------------------------
CREATE OR REPLACE FUNCTION EXTRACT_H1_FROM_CONTENT(content TEXT)
RETURNS TEXT AS $$
DECLARE
    line TEXT;
    title TEXT := 'Untitled';
BEGIN
    -- Iterate over each line in the content
    FOR line IN SELECT * FROM regexp_split_to_table(content, '\n') LOOP
        -- Trim leading and trailing spaces
        line := trim(line);
        -- Check if the line starts with a Markdown H1
        IF line LIKE '# %' THEN
            -- Set title and exit the loop
            title := substr(line, 3); -- Skip '# ' characters
            EXIT;
        END IF;
    END LOOP;

    RETURN title;
END;
$$ LANGUAGE plpgsql;

-- ***** Trigger --------------------------------------------------------------

CREATE OR REPLACE FUNCTION UPDATE_TITLE_FROM_CONTENT()
RETURNS TRIGGER AS $$
BEGIN
    -- Update the title based on extracted H1
    NEW.title := extract_h1_from_content(NEW.content);
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_title_from_content
BEFORE INSERT OR UPDATE ON notes
FOR EACH ROW EXECUTE FUNCTION UPDATE_TITLE_FROM_CONTENT();

-- ***** Heading Enforcement --------------------------------------------------
-- This way the title field is simply an endpoint for the H1 in the content
-- Enforce title as heading
CREATE OR REPLACE FUNCTION ENFORCE_READ_ONLY_TITLE()
RETURNS TRIGGER AS $$
BEGIN
    IF NEW.title IS DISTINCT FROM OLD.title THEN
        -- Prevent manual updates by reverting to the calculated title
        NEW.title := OLD.title;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER ensure_title_is_read_only
BEFORE UPDATE ON notes
FOR EACH ROW EXECUTE FUNCTION ENFORCE_READ_ONLY_TITLE();


-- *** Hierarchy --------------------------------------------------------------
CREATE TABLE note_hierarchy (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    parent_note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    child_note_id INT REFERENCES notes (id) ON DELETE CASCADE,
    -- This enforces that each child note can only have one parent
    UNIQUE (child_note_id)
);

-- In the future, consider another table for mapping a note_id
-- or a hierarchy_id to a specific type of relationship
-- e.g.: hierarchy_type TEXT CHECK (hierarchy_type IN ('page', 'block', 'subpage')),
-- This must be a separate table because the  tags_hierarchy
-- and task_hierarchy tables must have a common structure
-- for shared logic.


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
