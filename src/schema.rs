// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::query_builder::QueryId, Clone, diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "tsvector", schema = "pg_catalog"))]
    pub struct Tsvector;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Tsvector;

    assets (id) {
        id -> Int4,
        note_id -> Nullable<Int4>,
        asset_type -> Text,
        location -> Text,
        description -> Nullable<Text>,
        description_tsv -> Nullable<Tsvector>,
        created_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    attributes (id) {
        id -> Int4,
        name -> Text,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    note_attributes (id) {
        id -> Int4,
        note_id -> Nullable<Int4>,
        attribute_id -> Nullable<Int4>,
        value -> Text,
    }
}

diesel::table! {
    note_hierarchy (id) {
        id -> Int4,
        parent_note_id -> Nullable<Int4>,
        child_note_id -> Nullable<Int4>,
        hierarchy_type -> Nullable<Text>,
    }
}

diesel::table! {
    note_modifications (id) {
        id -> Int4,
        note_id -> Nullable<Int4>,
        previous_content -> Text,
        modified_at -> Nullable<Timestamp>,
    }
}

diesel::table! {
    note_type_mappings (note_id, type_id) {
        note_id -> Int4,
        type_id -> Int4,
    }
}

diesel::table! {
    note_types (id) {
        id -> Int4,
        name -> Text,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Tsvector;

    notes (id) {
        id -> Int4,
        title -> Text,
        content -> Text,
        created_at -> Nullable<Timestamp>,
        modified_at -> Nullable<Timestamp>,
        fts -> Nullable<Tsvector>,
    }
}

diesel::table! {
    tag_hierarchy (id) {
        id -> Int4,
        parent_tag_id -> Nullable<Int4>,
        child_tag_id -> Nullable<Int4>,
    }
}

diesel::table! {
    tags (id) {
        id -> Int4,
        name -> Text,
    }
}

diesel::joinable!(assets -> notes (note_id));
diesel::joinable!(note_attributes -> attributes (attribute_id));
diesel::joinable!(note_attributes -> notes (note_id));
diesel::joinable!(note_modifications -> notes (note_id));
diesel::joinable!(note_type_mappings -> note_types (type_id));
diesel::joinable!(note_type_mappings -> notes (note_id));

diesel::allow_tables_to_appear_in_same_query!(
    assets,
    attributes,
    note_attributes,
    note_hierarchy,
    note_modifications,
    note_type_mappings,
    note_types,
    notes,
    tag_hierarchy,
    tags,
);
