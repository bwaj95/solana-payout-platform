use sqlx::PgPool;

#[derive(Clone)]
pub struct SolanaDeployment {
    pub cluster: String,
    pub rpc_url: String,
    pub program_id: String,
}

// AppState is a typed container holding dependencies shared by HTTP handlers.
// It should contain infrastructure dependencies
#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub solana: SolanaDeployment,
}
// PgPool is not copied with all its connections each time it is cloned.
// It is a cheap shared handle to the same underlying pool.

impl AppState {
    pub fn new(
        db: PgPool,
        solana_cluster: String,
        solana_rpc_url: String,
        solana_program_id: String,
    ) -> Self {
        Self {
            db,
            solana: SolanaDeployment {
                cluster: solana_cluster,
                rpc_url: solana_rpc_url,
                program_id: solana_program_id,
            },
        }
    }
}
