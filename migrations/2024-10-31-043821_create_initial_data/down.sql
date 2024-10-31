-- Remove all sample data in reverse order of dependencies
DELETE FROM task_schedules;
DELETE FROM tasks;
DELETE FROM note_attributes;
DELETE FROM note_type_mappings;
DELETE FROM note_tags;
DELETE FROM note_hierarchy;
DELETE FROM note_modifications;
DELETE FROM notes;
DELETE FROM tag_hierarchy;
DELETE FROM tags;
