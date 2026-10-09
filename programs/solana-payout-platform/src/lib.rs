pub mod anchor_utils;
pub mod constants;
pub mod error;
pub mod events;
pub mod instructions;
pub mod payment_terms;
pub mod state;
use anchor_lang::prelude::*;

pub use anchor_utils::*;
pub use constants::*;
pub use instructions::*;
pub use payment_terms::*;
pub use state::*;

declare_id!("5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv");

#[program]
pub mod solana_payout_platform {

    use super::*;

    pub fn initialize_organization(
        ctx: Context<InitializeOrganization>,
        organization_id: u64,
        owner_member_id: u64,
    ) -> Result<()> {
        instructions::initialize_organization::initialize_organization_handler(
            ctx,
            organization_id,
            owner_member_id,
        )
    }

    pub fn create_member(
        ctx: Context<CreateMember>,
        member_id: u64,
        authorized_wallet: Pubkey,
        roles: u16,
    ) -> Result<()> {
        instructions::create_member::create_member_handler(ctx, member_id, authorized_wallet, roles)
    }

    pub fn create_policy_version(
        ctx: Context<CreatePolicyVersion>,
        policy_id: u64,
        version: u64,
        threshold: u8,
    ) -> Result<()> {
        instructions::create_policy_version::create_policy_version_handler(
            ctx, policy_id, version, threshold,
        )
    }

    pub fn initialize_vault(ctx: Context<InitializeVault>, vault_id: u64) -> Result<()> {
        instructions::initialize_vault::initialize_vault_handler(ctx, vault_id)
    }

    pub fn register_recipient(
        ctx: Context<RegisterRecipient>,
        recipient_id: u64,
        destination_wallet: Pubkey,
    ) -> Result<()> {
        instructions::register_recipient::register_recipient_handler(
            ctx,
            recipient_id,
            destination_wallet,
        )
    }

    pub fn create_payment(
        ctx: Context<CreatePayment>,
        payment_id: u64,
        amount: u64,
        execute_after: i64,
    ) -> Result<()> {
        instructions::create_payment::create_payment_handler(ctx, payment_id, amount, execute_after)
    }

    pub fn approve_payment(ctx: Context<ApprovePayment>, payment_id: u64) -> Result<()> {
        instructions::approve_payment::approve_payment_handler(ctx, payment_id)
    }

    pub fn finalize_payment_approval(
        ctx: Context<FinalizePaymentApproval>,
        payment_id: u64,
    ) -> Result<()> {
        instructions::finalize_payment_approval::finalize_payment_approval_handler(ctx, payment_id)
    }

    pub fn execute_spl_payment(ctx: Context<ExecuteSplPayment>, payment_id: u64) -> Result<()> {
        instructions::execute_spl_payment::execute_spl_payment_handler(ctx, payment_id)
    }

    pub fn cancel_payment(ctx: Context<CancelPayment>, payment_id: u64) -> Result<()> {
        instructions::cancel_payment::cancel_payment_handler(ctx, payment_id)
    }

    pub fn rotate_recipient_wallet(
        ctx: Context<RotateRecipientWallet>,
        recipient_id: u64,
        new_destination_wallet: Pubkey,
    ) -> Result<()> {
        instructions::rotate_recipient_wallet::rotate_recipient_wallet_handler(
            ctx,
            recipient_id,
            new_destination_wallet,
        )
    }

    pub fn set_organization_paused(
        ctx: Context<SetOrganizationPaused>,
        paused: bool,
    ) -> Result<()> {
        instructions::set_organization_paused::set_organization_paused_handler(ctx, paused)
    }

    pub fn withdraw_spl_vault_funds(
        ctx: Context<WithdrawSplVaultFunds>,
        amount: u64,
        destination_wallet: Pubkey,
    ) -> Result<()> {
        instructions::withdraw_spl_vault_funds::withdraw_spl_vault_funds_handler(
            ctx,
            amount,
            destination_wallet,
        )
    }
}
