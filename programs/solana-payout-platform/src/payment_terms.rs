use anchor_lang::prelude::*;
use solana_sha256_hasher::hashv;

use crate::{SettlementRail, PAYMENT_TERMS_DOMAIN};

#[allow(clippy::too_many_arguments)]
pub fn compute_payment_terms_hash(
    organization: &Pubkey,
    payment_id: u64,
    payment_revision: u32,
    recipient: &Pubkey,
    destination: &Pubkey,
    recipient_wallet_revision: u32,
    vault: &Pubkey,
    mint: &Pubkey,
    amount: u64,
    policy_version: &Pubkey,
    settlement_rail: SettlementRail,
    execute_after: i64,
) -> [u8; 32] {
    let payment_id_bytes = payment_id.to_le_bytes();
    let payment_revision_bytes = payment_revision.to_le_bytes();
    let recipient_wallet_revision_bytes = recipient_wallet_revision.to_le_bytes();
    let amount_bytes = amount.to_le_bytes();
    let settlement_rail_bytes = [settlement_rail.hash_tag()];
    let execute_after_bytes = execute_after.to_le_bytes();

    let digest = hashv(&[
        PAYMENT_TERMS_DOMAIN,
        organization.as_ref(),
        &payment_id_bytes,
        &payment_revision_bytes,
        recipient.as_ref(),
        destination.as_ref(),
        &recipient_wallet_revision_bytes,
        vault.as_ref(),
        mint.as_ref(),
        &amount_bytes,
        policy_version.as_ref(),
        &settlement_rail_bytes,
        &execute_after_bytes,
    ]);

    *digest.as_bytes()
}
