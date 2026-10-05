use solana_keypair::Keypair;
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_USER_ID: AtomicU64 = AtomicU64::new(1);
pub struct User {
    id: u64,
    name: String,
    keypair: Keypair,
}

impl User {
    pub fn new(name: &str) -> Self {
        Self {
            id: NEXT_USER_ID.fetch_add(1, Ordering::SeqCst),
            name: name.to_owned(),
            keypair: Keypair::new(),
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn pubkey(&self) -> Pubkey {
        self.keypair.pubkey()
    }

    pub fn signer(&self) -> &Keypair {
        &self.keypair
    }
}
