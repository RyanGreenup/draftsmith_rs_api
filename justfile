reset-db:
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -c "DROP DATABASE draftsmith2"
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -c "CREATE DATABASE draftsmith2"
    rm -rf ./uploads
    diesel migration run

server-down:
    fuser 37240/tcp -k

server-up:
    just reset-db && clear && RUST_DEBUG=1 cargo run --bin cli serve -a 0.0.0.0:37240
server-up-disown:
    just server-up & disown

test:
    just server-down
    just server-up-disown
    cargo test

psql:
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -d draftsmith2


