# config

Currently the server is configured entirely through environment
variables (`OPENPARTS_DATA_DIR`, `PORT`) -- see `src/main.rs`. This
directory is reserved for future file-based configuration (database
connection settings, rate limiting, object storage credentials) once
those features exist; it is intentionally empty for now.
