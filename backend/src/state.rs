use sqlx::PgPool;

// AppState is a typed container holding dependencies shared by HTTP handlers.
// It should contain infrastructure dependencies
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
}
// PgPool is not copied with all its connections each time it is cloned.
// It is a cheap shared handle to the same underlying pool.

impl AppState {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }
}
