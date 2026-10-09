use anchor_lang::prelude::*;

use crate::{
    error::ErrorCode, events::PaymentCancelled, Member, Organization, Payment, PaymentState,
    ReservationState, VaultState, MEMBER_SEED, ORGANIZATION_SEED, PAYMENT_SEED, ROLE_ADMIN,
    VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(payment_id: u64)]
pub struct CancelPayment<'info> {
    pub authority: Signer<'info>,

    /*
     * Cancellation must remain available while the organization is paused.
     * Pausing should stop financial execution, not block recovery operations.
     */
    #[account(
        seeds = [
            ORGANIZATION_SEED,
            organization.creator.as_ref(),
            organization.organization_id.to_le_bytes().as_ref(),
        ],
        bump = organization.bump,
    )]
    pub organization: Account<'info, Organization>,

    #[account(
        seeds = [
            MEMBER_SEED,
            organization.key().as_ref(),
            admin_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = admin_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = admin_member.active
            @ ErrorCode::InactiveMember,
        constraint = admin_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,
        constraint = (admin_member.roles & ROLE_ADMIN) != 0
            @ ErrorCode::MissingAdminRole,
    )]
    pub admin_member: Account<'info, Member>,

    /*
     * The vault must be writable because an Approved + Held payment releases
     * its amount from the aggregate reserved_total.
     *
     * We deliberately do not require vault_state.active. An inactive vault
     * must still allow existing reservations to be released.
     */
    #[account(
        mut,
        seeds = [
            VAULT_SEED,
            organization.key().as_ref(),
            vault_state.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault_state.bump,
        has_one = organization
            @ ErrorCode::VaultOrganizationMismatch,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [
            PAYMENT_SEED,
            organization.key().as_ref(),
            payment.payment_id.to_le_bytes().as_ref(),
        ],
        bump = payment.bump,
        has_one = organization
            @ ErrorCode::PaymentTermsMismatch,
        constraint = payment.payment_id == payment_id
            @ ErrorCode::InvalidPaymentId,
        constraint = payment.vault == vault_state.key()
            @ ErrorCode::PaymentTermsMismatch,
    )]
    pub payment: Account<'info, Payment>,
}

pub fn cancel_payment_handler(ctx: Context<CancelPayment>, _payment_id: u64) -> Result<()> {
    let previous_payment_state = ctx.accounts.payment.payment_state.clone();

    let previous_reservation_state = ctx.accounts.payment.reservation_state.clone();

    let current_reserved_total = ctx.accounts.vault_state.reserved_total;

    /*
     * Validate the payment and reservation states together.
     *
     * Valid combinations:
     *
     * PendingApproval + None:
     *     No funds were reserved, so nothing is released.
     *
     * AwaitingFunds + None:
     *     Approval threshold was reached, but reservation failed because the
     *     vault lacked funds. Again, nothing is released.
     *
     * Approved + Held:
     *     The payment amount is part of reserved_total and must be released.
     *
     * All other combinations represent a terminal or inconsistent state.
     */
    let (released_amount, remaining_reserved_total, final_reservation_state) =
        match (&previous_payment_state, &previous_reservation_state) {
            (PaymentState::PendingApproval, ReservationState::None)
            | (PaymentState::AwaitingFunds, ReservationState::None) => {
                (0, current_reserved_total, ReservationState::None)
            }

            (PaymentState::Approved, ReservationState::Held) => {
                let remaining_reserved_total = current_reserved_total
                    .checked_sub(ctx.accounts.payment.amount)
                    .ok_or(ErrorCode::VaultReservationInsufficient)?;

                (
                    ctx.accounts.payment.amount,
                    remaining_reserved_total,
                    ReservationState::Released,
                )
            }

            _ => {
                return err!(ErrorCode::InvalidPaymentCancellationState);
            }
        };

    /*
     * No CPI is needed. Cancellation only changes program-owned accounting.
     * These mutations remain atomic with the entire instruction.
     */
    ctx.accounts.vault_state.reserved_total = remaining_reserved_total;

    ctx.accounts.payment.payment_state = PaymentState::Cancelled;

    ctx.accounts.payment.reservation_state = final_reservation_state.clone();

    let cancelled_at = Clock::get()?.unix_timestamp;

    emit!(PaymentCancelled {
        organization: ctx.accounts.organization.key(),
        payment: ctx.accounts.payment.key(),
        payment_id: ctx.accounts.payment.payment_id,
        payment_revision: ctx.accounts.payment.payment_revision,

        vault: ctx.accounts.vault_state.key(),

        cancelled_by_member: ctx.accounts.admin_member.key(),
        cancelled_by_wallet: ctx.accounts.authority.key(),

        previous_payment_state,
        previous_reservation_state,
        final_reservation_state,

        released_amount,
        remaining_reserved_total,
        cancelled_at,
    });

    Ok(())
}
