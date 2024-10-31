-- DROP DATABASE IF EXISTS draftsmith;
-- CREATE DATABASE draftsmith;
-- \c draftsmith;

-- Enable full-text search
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS unaccent;

-- Table to store notes with a full-text search
CREATE TABLE notes (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    fts tsvector
);

-- Trigger to update the full-text search vector
CREATE TRIGGER notes_fts_update
BEFORE INSERT OR UPDATE ON notes
FOR EACH ROW EXECUTE PROCEDURE tsvector_update_trigger(
    fts, 'pg_catalog.english', title, content
);

-- Table to store modified dates
CREATE TABLE note_modifications (
    note_id INT REFERENCES notes(id),
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Table for categories
CREATE TABLE categories (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

-- Table for tags
CREATE TABLE tags (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL
);

-- Many-to-many relationship tables for categories and tags
CREATE TABLE note_categories (
    note_id INT REFERENCES notes(id),
    category_id INT REFERENCES categories(id),
    PRIMARY KEY (note_id, category_id)
);

-- Tags have heirarchy
CREATE TABLE tag_hierarchy (
    id SERIAL PRIMARY KEY,
    parent_tag_id INT REFERENCES tags(id),
    child_tag_id INT REFERENCES tags(id),
    UNIQUE (child_tag_id)  -- Tags can only have one parent
);


CREATE TABLE note_tags (
    note_id INT REFERENCES notes(id),
    tag_id INT REFERENCES tags(id),
    PRIMARY KEY (note_id, tag_id)
);

-- Table for assets
CREATE TABLE assets (
 id SERIAL PRIMARY KEY,
 note_id INT REFERENCES notes(id),
 asset_type TEXT NOT NULL,
 location TEXT NOT NULL UNIQUE,
 description TEXT,
 description_tsv tsvector,
 created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP  -- Add this line
);


-- Create a function to automatically update the tsvector column
CREATE FUNCTION assets_description_trigger() RETURNS trigger AS $$
BEGIN
  NEW.description_tsv := to_tsvector('english', NEW.description);
  RETURN NEW;
END
$$ LANGUAGE plpgsql;

-- Create a trigger to call the function before insert or update
CREATE TRIGGER assets_description_update
BEFORE INSERT OR UPDATE ON assets
FOR EACH ROW EXECUTE FUNCTION assets_description_trigger();

-- Create an index on the tsvector column
CREATE INDEX assets_description_tsv_idx ON assets USING gin(description_tsv);

-- Table for misc attributes
CREATE TABLE attributes (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE note_attributes (
    id SERIAL PRIMARY KEY,
    note_id INT REFERENCES notes(id),
    attribute_id INT REFERENCES attributes(id),
    VALUE TEXT NOT NULL
);

-- Table for note types
CREATE TABLE note_types (
    id SERIAL PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    description TEXT
);

CREATE TABLE note_type_mappings (
    note_id INT REFERENCES notes(id),
    type_id INT REFERENCES note_types(id),
    PRIMARY KEY (note_id, type_id)
);

-- Table for handling note hierarchy
CREATE TABLE note_hierarchy (
    id SERIAL PRIMARY KEY,
    parent_note_id INT REFERENCES notes(id),
    child_note_id INT REFERENCES notes(id),
    hierarchy_type TEXT CHECK (hierarchy_type IN ('page', 'block', 'subpage')),
    UNIQUE (child_note_id)  -- This enforces that each child note can only have one parent
);

-- Table for journal/calendar view (optional)
CREATE TABLE journal_entries (
    id SERIAL PRIMARY KEY,
    note_id INT REFERENCES notes(id),
    entry_date DATE NOT NULL
);


-- Task Management

-- Track notes as task objects

CREATE TABLE tasks (
    id SERIAL PRIMARY KEY,                 -- Unique task identifier
    note_id INT REFERENCES notes(id) ON DELETE CASCADE, -- Link to notes
    status TEXT CHECK (status IN ('todo', 'done', 'wait', 'hold', 'idea', 'kill', 'proj', 'event')), -- Status of the task
    effort_estimate NUMERIC,              -- Estimated effort in hours
    actual_effort NUMERIC,                -- Actual effort in hours
    deadline TIMESTAMP,                   -- Deadline for the task
    priority INT CHECK (priority IS NULL OR priority BETWEEN 1 AND 5), -- Priority of the task
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    all_day BOOLEAN DEFAULT FALSE,  -- Flag for all-day events (e.g. Daylight Saving savings on this day)
    goal_relationship INT CHECK (goal_relationship IS NULL OR goal_relationship BETWEEN 1 AND 5), -- Relationship to goals
    UNIQUE (note_id)  -- A note can only be a task once, otherwise conflicts arise with schedule etc.
);

-- Schedule tasks over certain days

CREATE TABLE task_schedules (
    id SERIAL PRIMARY KEY,                 -- Unique schedule identifier
    task_id INT REFERENCES tasks(id) ON DELETE CASCADE, -- Link to tasks
    start_datetime TIMESTAMP,              -- Scheduled start datetime
    end_datetime TIMESTAMP                 -- Scheduled end datetime
);


-- Clock Table (consider generalizing this so that notes can have clock tables too)
CREATE TABLE task_clocks (
    id SERIAL PRIMARY KEY,                 -- Unique clock identifier
    task_id INT REFERENCES tasks(id) ON DELETE CASCADE, -- Link to tasks
    clock_in TIMESTAMP,                    -- Clock in time
    clock_out TIMESTAMP                    -- Clock out time
);


-- Populate initial data for note types
INSERT INTO note_types (name, description) VALUES
    ('asset', 'Asset related notes'),
    ('bookmark', 'Bookmark related notes'),
    ('contact', 'Contact information'),
    ('page', 'A standalone page'),
    ('block', 'A block of information within a page'),
    ('subpage', 'A subpage within a note');

-- Populate initial data for categories
INSERT INTO categories (name) VALUES
    ('Personal'),
    ('Work'),
    ('Ideas'),
    ('Journal');

-- Populate initial data for tags
INSERT INTO tags (name) VALUES
    ('important'),
    ('urgent'),
    ('todo'),
    ('done');

-- Populate initial data for attributes
INSERT INTO attributes (name, description) VALUES
    ('location', 'Location of the note'),
    ('author', 'Author of the note'),
    ('source', 'Source of the note');

-- Populate initial data for notes
INSERT INTO notes (title, content) VALUES
    ('First note', 'This is the first note in the system.'),
    ('Second note', 'This is the second note in the system.'),
    ('Third note', 'Note Number Three.');


-- Populate some hierarchy data
INSERT INTO note_hierarchy (parent_note_id, child_note_id, hierarchy_type) VALUES
    (1, 2, 'block'),
    (2, 3, 'block');

-- Populate some task data
INSERT INTO tasks (note_id, status, effort_estimate, actual_effort, deadline, priority, all_day, goal_relationship) VALUES
    (1, 'todo', 1.5, 0, '2021-12-31 23:59:59', 3, FALSE, 3),
    (2, 'done', 0.5, 0.5, '2021-12-31 23:59:59', 2, FALSE, 2),
    (3, 'todo', 2, 0, '2021-12-31 23:59:59', 1, FALSE, 1);
