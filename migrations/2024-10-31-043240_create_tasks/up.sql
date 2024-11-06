-- Your SQL goes here
CREATE TABLE tasks (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    note_id INT REFERENCES notes (id) ON DELETE CASCADE, -- Link to notes
    -- Status of the task
    status TEXT NOT NULL CHECK (
        status IN (
            'todo', 'done', 'wait', 'hold', 'idea', 'kill', 'proj', 'event'
        )
    ),
    effort_estimate NUMERIC,              -- Estimated effort in hours
    actual_effort NUMERIC,                -- Actual effort in hours
    deadline TIMESTAMP,                   -- Deadline for the task
    -- Priority of the task
    priority INT CHECK (priority IS NULL OR priority BETWEEN 1 AND 5),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    modified_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    -- Flag for all-day events (e.g. Daylight Saving savings on this day)
    all_day BOOLEAN DEFAULT FALSE,
    -- Relationship to goals
    goal_relationship INT CHECK (
        goal_relationship IS NULL OR goal_relationship BETWEEN 1 AND 5
    ),
    -- A note can only be a task once,
    -- otherwise conflicts arise with schedule etc.
    UNIQUE (note_id)
);



-- Auto update the modified_at column
CREATE OR REPLACE FUNCTION UPDATE_MODIFIED_AT_COLUMN_ON_TASKS()
RETURNS TRIGGER AS $$
BEGIN
    NEW.modified_at = CURRENT_TIMESTAMP;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER set_modified_at
BEFORE UPDATE ON tasks
FOR EACH ROW
EXECUTE FUNCTION UPDATE_MODIFIED_AT_COLUMN_ON_TASKS();

-- Tasks can have hierarchy

CREATE TABLE task_hierarchy (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    parent_task_id INT REFERENCES tasks (id),
    child_task_id INT REFERENCES tasks (id),
    UNIQUE (child_task_id)  -- Tags can only have one parent
);

-- Schedule tasks over certain days
CREATE TABLE task_schedules (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    -- Link to tasks
    task_id INT NOT NULL REFERENCES tasks (id) ON DELETE CASCADE,
    start_datetime TIMESTAMP,              -- Scheduled start datetime
    end_datetime TIMESTAMP                 -- Scheduled end datetime
);


-- Clock Table (consider generalizing this so that notes can
--              have clock tables too)
CREATE TABLE task_clocks (
    id INT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    task_id INT REFERENCES tasks (id) ON DELETE CASCADE,
    clock_in TIMESTAMP NOT NULL,
    clock_out TIMESTAMP CHECK (clock_out > clock_in)
);
