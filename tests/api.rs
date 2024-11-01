use axum::{
    body::Body,
    http::{Request, StatusCode},
    routing::Router,
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use serde_json::{json, Value};
use tower::ServiceExt;

type Pool = r2d2::Pool<ConnectionManager<PgConnection>>;

fn setup() -> (Router, Pool) {
    dotenv::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    // Clear the database before each test
    let mut conn = pool.get().unwrap();
    diesel::delete(schema::notes::table)
        .execute(&mut conn)
        .unwrap();

    let app = api::create_router(pool.clone());
    (app, pool)
}

#[tokio::test]
async fn test_notes_crud() {
    let (app, _pool) = setup();

    // Test creating a note
    let create_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/notes/flat")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Test Note",
                        "content": "This is a test note"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);

    let create_body = axum::body::to_bytes(create_response.into_body())
        .await
        .unwrap();
    let create_json: Value = serde_json::from_slice(&create_body).unwrap();
    let note_id = create_json["id"].as_i64().unwrap();

    // Test getting all notes
    let list_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/notes/flat")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);

    let list_body = axum::body::to_bytes(list_response.into_body())
        .await
        .unwrap();
    let list_json: Value = serde_json::from_slice(&list_body).unwrap();
    assert!(list_json.as_array().unwrap().len() > 0);

    // Test getting a single note
    let get_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/notes/flat/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_response.status(), StatusCode::OK);

    // Test updating a note
    let update_response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/notes/flat/{}", note_id))
                .header("Content-Type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Updated Test Note",
                        "content": "This note has been updated"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_response.status(), StatusCode::OK);

    // Test deleting a note
    let delete_response = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/notes/flat/{}", note_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::OK);
}
