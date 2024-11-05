reset-db:
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -c "DROP DATABASE draftsmith2"
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -c "CREATE DATABASE draftsmith2"
    rm -rf ./uploads
    diesel migration run

