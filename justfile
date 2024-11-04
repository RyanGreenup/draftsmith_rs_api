reset-db:
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -c "DROP DATABASE draftsmith2"
    PGPASSWORD=postgres psql -h localhost -p 5432 -U postgres -c "CREATE DATABASE draftsmith2"
    PGPASSWORD=postgres psql -h localhost -p 5434 -U postgres -c "CREATE DATABASE draftsmith2"
    PGPASSWORD=postgres psql -h localhost -p 5434 -U postgres -c "DROP DATABASE draftsmith2"
    PGPASSWORD=postgres psql -h localhost -p 5434 -U postgres -c "CREATE DATABASE draftsmith2"
    diesel migration run
