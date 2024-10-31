-- First create foundational tags
INSERT INTO tags (name)
VALUES
('work'),
('personal'),
('reference'),
('meeting'),
('project'),
('documentation'),
('template');

-- Create tag hierarchies
INSERT INTO tag_hierarchy (parent_tag_id, child_tag_id)
SELECT
    p.id AS parent_id,
    c.id AS child_id
FROM tags AS p, tags AS c
WHERE
    (p.name = 'work' AND c.name = 'meeting')
    OR (p.name = 'reference' AND c.name = 'documentation');

-- Create initial welcome and guide notes
INSERT INTO notes (title, content)
VALUES
(
    'Welcome to DraftSmith',
    E'Welcome to your new note-taking system!\n\n'
    || 'DraftSmith helps you organize your thoughts, tasks, and knowledge. '
    || 'This note will show you the basic features available.\n\n'
    || 'Key Features:\n'
    || '- Hierarchical notes\n'
    || '- Tags and categories\n'
    || '- Task management\n'
    || '- Full-text search\n'
    || '- Asset attachments'
),
(
    'Getting Started Guide',
    E'# Quick Start Guide\n\n'
    || '1. **Creating Notes**\n'
    || '   - Use the CLI to create new notes\n'
    || '   - Notes support markdown formatting\n\n'
    || '2. **Organization**\n'
    || '   - Tag notes for easy finding\n'
    || '   - Create hierarchies with parent/child relationships\n\n'
    || '3. **Tasks**\n'
    || '   - Convert any note to a task\n'
    || '   - Set priorities and deadlines\n'
    || '   - Track time spent\n\n'
    || '4. **Search**\n'
    || '   - Full-text search across all notes\n'
    || '   - Filter by tags or attributes'
),
(
    'Meeting Note Template',
    E'# Meeting: [Title]\n'
    || 'Date: [Date]\n'
    || 'Participants: [Names]\n\n'
    || '## Agenda\n'
    || '1. \n2. \n3. \n\n'
    || '## Discussion\n\n'
    || '## Action Items\n'
    || '- [ ] \n'
    || '- [ ] \n\n'
    || '## Next Steps\n\n'
    || '## Notes'
),
(
    'Project Planning Template',
    E'# Project: [Name]\n\n'
    || '## Overview\n'
    || '[Brief description]\n\n'
    || '## Objectives\n'
    || '1. \n2. \n\n'
    || '## Timeline\n'
    || '- Start: \n'
    || '- Milestones:\n'
    || '- Deadline: \n\n'
    || '## Resources\n\n'
    || '## Risks\n\n'
    || '## Status Updates'
);

-- Create note hierarchies
INSERT INTO note_hierarchy (
    parent_note_id,
    child_note_id,
    hierarchy_type
)
SELECT
    p.id AS parent_note_id,
    c.id AS child_note_id,
    'subpage' AS hierarchy_type
FROM notes AS p, notes AS c
WHERE
    p.title = 'Welcome to DraftSmith'
    AND c.title = 'Getting Started Guide';

-- Add note types
INSERT INTO note_type_mappings (note_id, type_id)
SELECT
    n.id AS note_id,
    t.id AS type_id
FROM notes AS n, note_types AS t
WHERE
    (n.title = 'Meeting Note Template' AND t.name = 'template')
    OR (n.title = 'Project Planning Template' AND t.name = 'template')
    OR (n.title = 'Welcome to DraftSmith' AND t.name = 'page')
    OR (n.title = 'Getting Started Guide' AND t.name = 'page');

-- Tag the notes
INSERT INTO note_tags (note_id, tag_id)
SELECT
    n.id AS note_id,
    t.id AS tag_id
FROM notes AS n, tags AS t
WHERE
    (n.title = 'Welcome to DraftSmith' AND t.name = 'documentation')
    OR (n.title = 'Meeting Note Template' AND t.name IN ('template', 'meeting'))
    OR (
        n.title = 'Project Planning Template'
        AND t.name IN ('template', 'project')
    );

-- Create a sample task from the project template
INSERT INTO tasks (
    note_id,
    status,
    effort_estimate,
    priority,
    deadline,
    goal_relationship
)
SELECT
    id AS note_id,
    'proj' AS status,
    40 AS effort_estimate,
    3 AS priority,
    CURRENT_DATE + INTERVAL '30 days' AS deadline,
    4 AS goal_relationship
FROM notes
WHERE title = 'Project Planning Template';

-- Add a schedule for the task
INSERT INTO task_schedules (
    task_id,
    start_datetime,
    end_datetime
)
SELECT
    t.id AS task_id,
    CURRENT_TIMESTAMP AS start_datetime,
    CURRENT_TIMESTAMP + INTERVAL '30 days' AS end_datetime
FROM tasks AS t
INNER JOIN notes AS n ON t.note_id = n.id
WHERE n.title = 'Project Planning Template';

-- Add attributes to notes
INSERT INTO note_attributes (note_id, attribute_id, value)
SELECT
    n.id AS note_id,
    a.id AS attribute_id,
    'System' AS attr_value
FROM notes AS n, attributes AS a
WHERE
    n.title IN ('Welcome to DraftSmith', 'Getting Started Guide')
    AND a.name = 'author';
