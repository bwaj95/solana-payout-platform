use std::str::FromStr;

use solana_pubkey::Pubkey;
use thiserror::Error;

const ORGANIZATION_SEED: &[u8] = b"organization";
const MEMBER_SEED: &[u8] = b"member";
const MEMBER_WALLET_SEED: &[u8] = b"member_wallet";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializeOrganizationPdas {
    pub organization_pda: String,
    pub organization_bump: u8,

    pub owner_member_pda: String,
    pub owner_member_bump: u8,

    pub owner_member_wallet_pda: String,
    pub owner_member_wallet_bump: u8,
}

#[derive(Debug, Error)]
pub enum PdaDerivationError {
    #[error("invalid Solana program ID")]
    InvalidProgramId,

    #[error("invalid creator wallet address")]
    InvalidCreatorWallet,

    #[error("could not derive {account_kind} PDA")]
    DerivationFailed { account_kind: &'static str },
}

pub fn derive_initialize_organization_pdas(
    program_id: &str,
    creator_wallet: &str,
    organization_id: u64,
    owner_member_id: u64,
) -> Result<InitializeOrganizationPdas, PdaDerivationError> {
    let program_id =
        Pubkey::from_str(program_id).map_err(|_| PdaDerivationError::InvalidProgramId)?;

    let creator =
        Pubkey::from_str(creator_wallet).map_err(|_| PdaDerivationError::InvalidCreatorWallet)?;

    /*
     * The byte arrays must remain alive while their references are
     * passed as PDA seeds.
     */
    let organization_id_bytes = organization_id.to_le_bytes();

    let (organization_pda, organization_bump) = Pubkey::try_find_program_address(
        &[
            ORGANIZATION_SEED,
            creator.as_ref(),
            organization_id_bytes.as_ref(),
        ],
        &program_id,
    )
    .ok_or(PdaDerivationError::DerivationFailed {
        account_kind: "Organization",
    })?;

    let owner_member_id_bytes = owner_member_id.to_le_bytes();

    let (owner_member_pda, owner_member_bump) = Pubkey::try_find_program_address(
        &[
            MEMBER_SEED,
            organization_pda.as_ref(),
            owner_member_id_bytes.as_ref(),
        ],
        &program_id,
    )
    .ok_or(PdaDerivationError::DerivationFailed {
        account_kind: "Member",
    })?;

    let (owner_member_wallet_pda, owner_member_wallet_bump) = Pubkey::try_find_program_address(
        &[
            MEMBER_WALLET_SEED,
            organization_pda.as_ref(),
            creator.as_ref(),
        ],
        &program_id,
    )
    .ok_or(PdaDerivationError::DerivationFailed {
        account_kind: "MemberWallet",
    })?;

    Ok(InitializeOrganizationPdas {
        organization_pda: organization_pda.to_string(),
        organization_bump,

        owner_member_pda: owner_member_pda.to_string(),
        owner_member_bump,

        owner_member_wallet_pda: owner_member_wallet_pda.to_string(),
        owner_member_wallet_bump,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROGRAM_ID: &str = "5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv";

    const CREATOR: &str = "4k5kaSZ796EWM8hNfXuhcdVbZ2MNWonTbRmsshWDBX7b";

    #[test]
    fn derivation_matches_known_devnet_organization() {
        /*
         * This organization was previously created by the devnet
         * end-to-end script. It gives us an independent fixture
         * produced by the TypeScript client and the deployed program.
         */
        let pdas =
            derive_initialize_organization_pdas(PROGRAM_ID, CREATOR, 1_791_977_588_031_017, 1)
                .expect("PDA derivation should succeed");

        assert_eq!(
            pdas.organization_pda,
            "8s7iXXXsx76jY4ipFYKGNBwihMuP8nKdub4sq69nModj"
        );
    }

    #[test]
    fn derivation_is_deterministic() {
        let first = derive_initialize_organization_pdas(PROGRAM_ID, CREATOR, 42, 7)
            .expect("first derivation should succeed");

        let second = derive_initialize_organization_pdas(PROGRAM_ID, CREATOR, 42, 7)
            .expect("second derivation should succeed");

        assert_eq!(first, second);
    }

    #[test]
    fn changing_organization_id_changes_all_child_pdas() {
        let first = derive_initialize_organization_pdas(PROGRAM_ID, CREATOR, 42, 7)
            .expect("first derivation should succeed");

        let second = derive_initialize_organization_pdas(PROGRAM_ID, CREATOR, 43, 7)
            .expect("second derivation should succeed");

        assert_ne!(first.organization_pda, second.organization_pda);

        assert_ne!(first.owner_member_pda, second.owner_member_pda);

        assert_ne!(
            first.owner_member_wallet_pda,
            second.owner_member_wallet_pda
        );
    }

    #[test]
    fn invalid_program_id_is_rejected() {
        let error = derive_initialize_organization_pdas("not-a-program-id", CREATOR, 42, 7)
            .expect_err("invalid program ID must fail");

        assert!(matches!(error, PdaDerivationError::InvalidProgramId));
    }

    #[test]
    fn invalid_creator_wallet_is_rejected() {
        let error = derive_initialize_organization_pdas(PROGRAM_ID, "not-a-wallet", 42, 7)
            .expect_err("invalid creator must fail");

        assert!(matches!(error, PdaDerivationError::InvalidCreatorWallet));
    }
}
