use std::io;

use borsh::BorshDeserialize;
use thiserror::Error;

use crate::solana_rpc::{FinalizedAccountSnapshot, FinalizedAccountsLookup};

/*
 * Anchor account discriminators:
 *
 * first 8 bytes of SHA-256("account:<AccountName>")
 */
const ORGANIZATION_DISCRIMINATOR: [u8; 8] = [145, 38, 152, 251, 91, 57, 118, 160];

const MEMBER_DISCRIMINATOR: [u8; 8] = [54, 19, 162, 21, 29, 166, 17, 198];

const MEMBER_WALLET_DISCRIMINATOR: [u8; 8] = [225, 26, 52, 143, 86, 46, 6, 54];

/*
 * Expected immutable identity of the three initialized accounts.
 */
#[derive(Debug)]
pub struct InitializeOrganizationAccountsExpectation {
    pub program_id: String,

    pub organization_pda: String,
    pub owner_member_pda: String,
    pub owner_member_wallet_pda: String,

    pub organization_id: u64,
    pub owner_member_id: u64,
    pub creator_wallet: String,

    pub organization_bump: u8,
    pub owner_member_bump: u8,
    pub owner_member_wallet_bump: u8,
}

/*
 * Private mirror of the Anchor Organization account layout.
 *
 * Pubkeys are represented as [u8; 32] to avoid making the backend
 * depend on the entire Anchor/Solana program crate.
 */
#[derive(Debug, BorshDeserialize)]
struct RawOrganization {
    organization_id: u64,
    creator: [u8; 32],
    owner_member: [u8; 32],
    paused: bool,
    bump: u8,
}

#[derive(Debug, BorshDeserialize)]
struct RawMember {
    organization: [u8; 32],
    member_id: u64,

    authorized_wallet: [u8; 32],
    authorization_revision: u64,

    roles: u16,
    active: bool,
    bump: u8,
}

#[derive(Debug, BorshDeserialize)]
struct RawMemberWallet {
    organization: [u8; 32],
    member: [u8; 32],
    authorized_wallet: [u8; 32],
    bump: u8,
}

/*
 * Validated, normalized Organization state ready for persistence.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedOrganizationAccount {
    pub address: String,
    pub organization_id: u64,
    pub creator_wallet: String,
    pub owner_member_pda: String,
    pub paused: bool,
    pub bump: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMemberAccount {
    pub address: String,
    pub organization_pda: String,
    pub member_id: u64,

    pub authorized_wallet: String,
    pub authorization_revision: u64,

    pub roles: u16,
    pub active: bool,
    pub bump: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedMemberWalletAccount {
    pub address: String,
    pub organization_pda: String,
    pub member_pda: String,
    pub authorized_wallet: String,
    pub bump: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInitializeOrganizationAccounts {
    /*
     * This is the getMultipleAccounts response context, not the
     * transaction slot.
     */
    pub observed_slot: u64,

    pub organization: ValidatedOrganizationAccount,
    pub owner_member: ValidatedMemberAccount,
    pub owner_member_wallet: ValidatedMemberWalletAccount,
}

#[derive(Debug, Error)]
pub enum AccountValidationError {
    #[error("expected exactly three initialization accounts, received {actual}")]
    UnexpectedAccountCount { actual: usize },

    #[error("{account_kind} account {address} was not found")]
    MissingAccount {
        account_kind: &'static str,
        address: String,
    },

    #[error("{account_kind} address mismatch: expected {expected}, received {actual}")]
    AddressMismatch {
        account_kind: &'static str,
        expected: String,
        actual: String,
    },

    #[error("{account_kind} owner mismatch: expected {expected}, received {actual}")]
    OwnerMismatch {
        account_kind: &'static str,
        expected: String,
        actual: String,
    },

    #[error("{account_kind} account must not be executable")]
    UnexpectedExecutableAccount { account_kind: &'static str },

    #[error(
        "{account_kind} data must contain at least an eight-byte discriminator; received {actual} bytes"
    )]
    AccountDataTooShort {
        account_kind: &'static str,
        actual: usize,
    },

    #[error("{account_kind} discriminator mismatch: expected {expected:?}, received {actual:?}")]
    DiscriminatorMismatch {
        account_kind: &'static str,
        expected: [u8; 8],
        actual: [u8; 8],
    },

    #[error("failed to decode {account_kind} account data")]
    AccountDecode {
        account_kind: &'static str,

        #[source]
        source: io::Error,
    },

    #[error("{account_kind}.{field} mismatch: expected {expected}, received {actual}")]
    FieldMismatch {
        account_kind: &'static str,
        field: &'static str,
        expected: String,
        actual: String,
    },
}

pub fn validate_initialize_organization_accounts(
    lookup: &FinalizedAccountsLookup,
    expected: &InitializeOrganizationAccountsExpectation,
) -> Result<ValidatedInitializeOrganizationAccounts, AccountValidationError> {
    /*
     * The RPC request order must be:
     *
     * 0. Organization
     * 1. owner Member
     * 2. owner MemberWallet
     */
    let [organization_entry, member_entry, member_wallet_entry] = lookup.accounts.as_slice() else {
        return Err(AccountValidationError::UnexpectedAccountCount {
            actual: lookup.accounts.len(),
        });
    };

    let organization_snapshot = require_account(
        organization_entry,
        "Organization",
        &expected.organization_pda,
        &expected.program_id,
    )?;

    let member_snapshot = require_account(
        member_entry,
        "Member",
        &expected.owner_member_pda,
        &expected.program_id,
    )?;

    let member_wallet_snapshot = require_account(
        member_wallet_entry,
        "MemberWallet",
        &expected.owner_member_wallet_pda,
        &expected.program_id,
    )?;

    let organization: RawOrganization = decode_anchor_account(
        organization_snapshot,
        "Organization",
        ORGANIZATION_DISCRIMINATOR,
    )?;

    let member: RawMember = decode_anchor_account(member_snapshot, "Member", MEMBER_DISCRIMINATOR)?;

    let member_wallet: RawMemberWallet = decode_anchor_account(
        member_wallet_snapshot,
        "MemberWallet",
        MEMBER_WALLET_DISCRIMINATOR,
    )?;

    let organization_creator = encode_pubkey(organization.creator);

    let organization_owner_member = encode_pubkey(organization.owner_member);

    let member_organization = encode_pubkey(member.organization);

    let member_authorized_wallet = encode_pubkey(member.authorized_wallet);

    let member_wallet_organization = encode_pubkey(member_wallet.organization);

    let member_wallet_member = encode_pubkey(member_wallet.member);

    let member_wallet_authorized_wallet = encode_pubkey(member_wallet.authorized_wallet);

    require_field(
        "Organization",
        "organization_id",
        expected.organization_id.to_string(),
        organization.organization_id.to_string(),
    )?;

    require_field(
        "Organization",
        "creator",
        expected.creator_wallet.clone(),
        organization_creator.clone(),
    )?;

    require_field(
        "Organization",
        "owner_member",
        expected.owner_member_pda.clone(),
        organization_owner_member.clone(),
    )?;

    require_field(
        "Member",
        "organization",
        expected.organization_pda.clone(),
        member_organization.clone(),
    )?;

    require_field(
        "Member",
        "member_id",
        expected.owner_member_id.to_string(),
        member.member_id.to_string(),
    )?;

    require_field(
        "MemberWallet",
        "organization",
        expected.organization_pda.clone(),
        member_wallet_organization.clone(),
    )?;

    require_field(
        "MemberWallet",
        "member",
        expected.owner_member_pda.clone(),
        member_wallet_member.clone(),
    )?;

    require_field(
        "Organization",
        "bump",
        expected.organization_bump.to_string(),
        organization.bump.to_string(),
    )?;

    require_field(
        "Member",
        "bump",
        expected.owner_member_bump.to_string(),
        member.bump.to_string(),
    )?;

    require_field(
        "MemberWallet",
        "bump",
        expected.owner_member_wallet_bump.to_string(),
        member_wallet.bump.to_string(),
    )?;

    /*
     * These two accounts describe the same current wallet
     * authorization. They must agree with each other.
     *
     * We do not require this current wallet to equal the original
     * creator forever, because a future wallet-rotation instruction
     * may legitimately change both accounts after initialization.
     */
    require_field(
        "MemberWallet",
        "authorized_wallet",
        member_authorized_wallet.clone(),
        member_wallet_authorized_wallet.clone(),
    )?;

    Ok(ValidatedInitializeOrganizationAccounts {
        observed_slot: lookup.context_slot,

        organization: ValidatedOrganizationAccount {
            address: organization_snapshot.address.clone(),
            organization_id: organization.organization_id,
            creator_wallet: organization_creator,
            owner_member_pda: organization_owner_member,
            paused: organization.paused,
            bump: organization.bump,
        },

        owner_member: ValidatedMemberAccount {
            address: member_snapshot.address.clone(),
            organization_pda: member_organization,
            member_id: member.member_id,
            authorized_wallet: member_authorized_wallet,
            authorization_revision: member.authorization_revision,
            roles: member.roles,
            active: member.active,
            bump: member.bump,
        },

        owner_member_wallet: ValidatedMemberWalletAccount {
            address: member_wallet_snapshot.address.clone(),
            organization_pda: member_wallet_organization,
            member_pda: member_wallet_member,
            authorized_wallet: member_wallet_authorized_wallet,
            bump: member_wallet.bump,
        },
    })
}

fn require_account<'a>(
    entry: &'a Option<FinalizedAccountSnapshot>,
    account_kind: &'static str,
    expected_address: &str,
    expected_program_id: &str,
) -> Result<&'a FinalizedAccountSnapshot, AccountValidationError> {
    let snapshot = entry
        .as_ref()
        .ok_or_else(|| AccountValidationError::MissingAccount {
            account_kind,
            address: expected_address.to_owned(),
        })?;

    if snapshot.address != expected_address {
        return Err(AccountValidationError::AddressMismatch {
            account_kind,
            expected: expected_address.to_owned(),
            actual: snapshot.address.clone(),
        });
    }

    /*
     * A program-owned state account must be owned by our deployed
     * program. Merely existing at the expected address is not enough.
     */
    if snapshot.owner != expected_program_id {
        return Err(AccountValidationError::OwnerMismatch {
            account_kind,
            expected: expected_program_id.to_owned(),
            actual: snapshot.owner.clone(),
        });
    }

    /*
     * Organization, Member and MemberWallet are data accounts, not
     * executable program accounts.
     */
    if snapshot.executable {
        return Err(AccountValidationError::UnexpectedExecutableAccount { account_kind });
    }

    Ok(snapshot)
}

fn decode_anchor_account<T>(
    snapshot: &FinalizedAccountSnapshot,
    account_kind: &'static str,
    expected_discriminator: [u8; 8],
) -> Result<T, AccountValidationError>
where
    T: BorshDeserialize,
{
    if snapshot.data.len() < 8 {
        return Err(AccountValidationError::AccountDataTooShort {
            account_kind,
            actual: snapshot.data.len(),
        });
    }

    let mut actual_discriminator = [0_u8; 8];

    actual_discriminator.copy_from_slice(&snapshot.data[..8]);

    if actual_discriminator != expected_discriminator {
        return Err(AccountValidationError::DiscriminatorMismatch {
            account_kind,
            expected: expected_discriminator,
            actual: actual_discriminator,
        });
    }

    T::try_from_slice(&snapshot.data[8..]).map_err(|source| AccountValidationError::AccountDecode {
        account_kind,
        source,
    })
}

fn require_field(
    account_kind: &'static str,
    field: &'static str,
    expected: String,
    actual: String,
) -> Result<(), AccountValidationError> {
    if expected != actual {
        return Err(AccountValidationError::FieldMismatch {
            account_kind,
            field,
            expected,
            actual,
        });
    }

    Ok(())
}

fn encode_pubkey(bytes: [u8; 32]) -> String {
    bs58::encode(bytes).into_string()
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    const ORGANIZATION_ID: u64 = 42;
    const OWNER_MEMBER_ID: u64 = 7;

    /*
     * Deterministic fake public keys make it easy to build related
     * accounts without introducing random values into the tests.
     */
    fn key(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn address(byte: u8) -> String {
        bs58::encode(key(byte)).into_string()
    }

    fn expectation() -> InitializeOrganizationAccountsExpectation {
        InitializeOrganizationAccountsExpectation {
            program_id: address(9),

            organization_pda: address(2),
            owner_member_pda: address(3),
            owner_member_wallet_pda: address(4),

            organization_id: ORGANIZATION_ID,
            owner_member_id: OWNER_MEMBER_ID,
            creator_wallet: address(1),

            organization_bump: 201,
            owner_member_bump: 202,
            owner_member_wallet_bump: 203,
        }
    }

    fn organization_data(
        organization_id: u64,
        creator: [u8; 32],
        owner_member: [u8; 32],
        paused: bool,
        bump: u8,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&ORGANIZATION_DISCRIMINATOR);

        data.extend_from_slice(&organization_id.to_le_bytes());

        data.extend_from_slice(&creator);
        data.extend_from_slice(&owner_member);
        data.push(u8::from(paused));
        data.push(bump);

        data
    }

    #[allow(clippy::too_many_arguments)]
    fn member_data(
        organization: [u8; 32],
        member_id: u64,
        authorized_wallet: [u8; 32],
        authorization_revision: u64,
        roles: u16,
        active: bool,
        bump: u8,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&MEMBER_DISCRIMINATOR);
        data.extend_from_slice(&organization);

        data.extend_from_slice(&member_id.to_le_bytes());

        data.extend_from_slice(&authorized_wallet);

        data.extend_from_slice(&authorization_revision.to_le_bytes());

        data.extend_from_slice(&roles.to_le_bytes());
        data.push(u8::from(active));
        data.push(bump);

        data
    }

    fn member_wallet_data(
        organization: [u8; 32],
        member: [u8; 32],
        authorized_wallet: [u8; 32],
        bump: u8,
    ) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(&MEMBER_WALLET_DISCRIMINATOR);

        data.extend_from_slice(&organization);
        data.extend_from_slice(&member);
        data.extend_from_slice(&authorized_wallet);
        data.push(bump);

        data
    }

    fn account(address: String, owner: String, data: Vec<u8>) -> Option<FinalizedAccountSnapshot> {
        Some(FinalizedAccountSnapshot {
            address,
            owner,
            lamports: 1_000_000,
            executable: false,
            data,
        })
    }

    fn valid_lookup() -> FinalizedAccountsLookup {
        let expected = expectation();

        FinalizedAccountsLookup {
            context_slot: 10_005,

            accounts: vec![
                account(
                    expected.organization_pda,
                    expected.program_id.clone(),
                    organization_data(ORGANIZATION_ID, key(1), key(3), false, 201),
                ),
                account(
                    expected.owner_member_pda,
                    expected.program_id.clone(),
                    member_data(key(2), OWNER_MEMBER_ID, key(1), 1, 3, true, 202),
                ),
                account(
                    expected.owner_member_wallet_pda,
                    expected.program_id,
                    member_wallet_data(key(2), key(3), key(1), 203),
                ),
            ],
        }
    }

    fn account_mut(
        lookup: &mut FinalizedAccountsLookup,
        index: usize,
    ) -> &mut FinalizedAccountSnapshot {
        lookup.accounts[index]
            .as_mut()
            .expect("test account should exist")
    }

    #[test]
    fn account_discriminators_match_anchor_names() {
        let cases = [
            ("Organization", ORGANIZATION_DISCRIMINATOR),
            ("Member", MEMBER_DISCRIMINATOR),
            ("MemberWallet", MEMBER_WALLET_DISCRIMINATOR),
        ];

        for (account_name, expected_discriminator) in cases {
            let preimage = format!("account:{account_name}");
            let hash = Sha256::digest(preimage.as_bytes());

            assert_eq!(
                &hash[..8],
                expected_discriminator.as_slice(),
                "wrong discriminator for {account_name}"
            );
        }
    }

    #[test]
    fn valid_initialized_accounts_are_accepted() {
        let expected = expectation();
        let lookup = valid_lookup();

        let validated = validate_initialize_organization_accounts(&lookup, &expected)
            .expect("valid accounts should pass");

        assert_eq!(validated.observed_slot, 10_005);

        assert_eq!(validated.organization.organization_id, ORGANIZATION_ID);

        assert_eq!(validated.organization.creator_wallet, address(1));

        assert_eq!(validated.organization.owner_member_pda, address(3));

        assert_eq!(validated.owner_member.member_id, OWNER_MEMBER_ID);

        assert_eq!(validated.owner_member.organization_pda, address(2));

        assert_eq!(validated.owner_member.authorized_wallet, address(1));

        assert_eq!(validated.owner_member_wallet.member_pda, address(3));

        assert_eq!(validated.owner_member_wallet.authorized_wallet, address(1));
    }

    #[test]
    fn missing_expected_account_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        lookup.accounts[1] = None;

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("missing Member account must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::MissingAccount {
                account_kind: "Member",
                ..
            }
        ));
    }

    #[test]
    fn account_owned_by_another_program_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        account_mut(&mut lookup, 0).owner = address(99);

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("wrong program owner must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::OwnerMismatch {
                account_kind: "Organization",
                ..
            }
        ));
    }

    #[test]
    fn wrong_account_discriminator_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        account_mut(&mut lookup, 0).data[0] ^= 0xff;

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("wrong discriminator must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::DiscriminatorMismatch {
                account_kind: "Organization",
                ..
            }
        ));
    }

    #[test]
    fn wrong_organization_id_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        account_mut(&mut lookup, 0).data = organization_data(999, key(1), key(3), false, 201);

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("wrong organization ID must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::FieldMismatch {
                account_kind: "Organization",
                field: "organization_id",
                ..
            }
        ));
    }

    #[test]
    fn member_linked_to_another_organization_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        account_mut(&mut lookup, 1).data =
            member_data(key(88), OWNER_MEMBER_ID, key(1), 1, 3, true, 202);

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("Member organization mismatch must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::FieldMismatch {
                account_kind: "Member",
                field: "organization",
                ..
            }
        ));
    }

    #[test]
    fn member_wallet_linked_to_another_member_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        account_mut(&mut lookup, 2).data = member_wallet_data(key(2), key(77), key(1), 203);

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("MemberWallet member mismatch must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::FieldMismatch {
                account_kind: "MemberWallet",
                field: "member",
                ..
            }
        ));
    }

    #[test]
    fn member_wallet_authorization_must_match_member() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        account_mut(&mut lookup, 2).data = member_wallet_data(key(2), key(3), key(66), 203);

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("wallet authorization mismatch must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::FieldMismatch {
                account_kind: "MemberWallet",
                field: "authorized_wallet",
                ..
            }
        ));
    }

    #[test]
    fn malformed_borsh_account_data_is_rejected() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        /*
         * Preserve the correct discriminator but truncate the
         * serialized Organization fields.
         */
        account_mut(&mut lookup, 0).data.truncate(20);

        let error = validate_initialize_organization_accounts(&lookup, &expected)
            .expect_err("truncated Borsh data must be rejected");

        assert!(matches!(
            error,
            AccountValidationError::AccountDecode {
                account_kind: "Organization",
                ..
            }
        ));
    }

    #[test]
    fn newer_mutable_state_is_preserved() {
        let expected = expectation();
        let mut lookup = valid_lookup();

        /*
         * Simulate legitimate state changes that occurred after the
         * initialization transaction but before reconciliation.
         */
        account_mut(&mut lookup, 0).data =
            organization_data(ORGANIZATION_ID, key(1), key(3), true, 201);

        account_mut(&mut lookup, 1).data =
            member_data(key(2), OWNER_MEMBER_ID, key(55), 4, 64, false, 202);

        account_mut(&mut lookup, 2).data = member_wallet_data(key(2), key(3), key(55), 203);

        let validated = validate_initialize_organization_accounts(&lookup, &expected)
            .expect("consistent newer mutable state should be accepted");

        assert!(validated.organization.paused);

        assert_eq!(validated.owner_member.authorized_wallet, address(55));

        assert_eq!(validated.owner_member.authorization_revision, 4);

        assert_eq!(validated.owner_member.roles, 64);
        assert!(!validated.owner_member.active);
    }
}
