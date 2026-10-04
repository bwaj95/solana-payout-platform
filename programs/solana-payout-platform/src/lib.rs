pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv");

#[program]
pub mod solana_payout_platform {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
    }
}
