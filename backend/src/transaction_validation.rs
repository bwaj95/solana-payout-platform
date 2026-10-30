use serde_json::Value;
use thiserror::Error;

use crate::solana_rpc::{FinalizedTransaction, TransactionInstruction};

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

/*
 * First eight bytes of:
 *
 * SHA-256("global:initialize_organization")
 *
 * This is the Anchor discriminator for the deployed
 * initialize_organization instruction.
 */
const INITIALIZE_ORGANIZATION_DISCRIMINATOR: [u8; 8] = [21, 20, 253, 138, 250, 160, 119, 87];

/*
 * The facts that PostgreSQL expects the finalized transaction to prove.
 *
 * These values will eventually come from:
 *
 * - transaction_submissions;
 * - organizations;
 * - organization_members;
 * - derived PDA addresses;
 * - configured Solana deployment.
 */
#[derive(Debug)]
pub struct InitializeOrganizationExpectation {
    pub transaction_signature: String,
    pub program_id: String,

    pub organization_id: u64,
    pub owner_member_id: u64,

    pub creator_wallet: String,
    pub organization_pda: String,
    pub owner_member_pda: String,
    pub owner_member_wallet_pda: String,
}

/*
 * Evidence returned after every validation has passed.
 *
 * The reconciler can safely use this information when writing the
 * finalized chain projections.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedInitializeOrganization {
    pub transaction_slot: u64,
    pub block_time: Option<i64>,
}

#[derive(Debug, Error)]
pub enum TransactionValidationError {
    #[error("finalized transaction contained no signatures")]
    MissingTransactionSignature,

    #[error("expected transaction signature {expected}, but RPC returned {actual}")]
    TransactionSignatureMismatch { expected: String, actual: String },

    #[error("finalized transaction failed during execution: {error}")]
    TransactionExecutionFailed { error: Value },

    #[error("expected creator wallet {0} was not a transaction signer")]
    CreatorDidNotSign(String),

    #[error("initialize_organization instruction for program {program_id} was not found")]
    InitializeOrganizationInstructionNotFound { program_id: String },

    #[error("found {count} initialize_organization instructions for program {program_id}")]
    MultipleInitializeOrganizationInstructions { program_id: String, count: usize },

    #[error(
        "initialize_organization accounts did not match; expected {expected:?}, received {actual:?}"
    )]
    InstructionAccountsMismatch {
        expected: Vec<String>,
        actual: Vec<String>,
    },

    #[error("initialize_organization data must contain exactly 24 bytes, received {actual}")]
    InvalidInstructionDataLength { actual: usize },

    #[error("expected organization ID {expected}, but instruction contained {actual}")]
    OrganizationIdMismatch { expected: u64, actual: u64 },

    #[error("expected owner member ID {expected}, but instruction contained {actual}")]
    OwnerMemberIdMismatch { expected: u64, actual: u64 },
}

pub fn validate_initialize_organization_transaction(
    transaction: &FinalizedTransaction,
    expected: &InitializeOrganizationExpectation,
) -> Result<ValidatedInitializeOrganization, TransactionValidationError> {
    let actual_signature = transaction
        .signatures
        .first()
        .ok_or(TransactionValidationError::MissingTransactionSignature)?;

    if actual_signature != &expected.transaction_signature {
        return Err(TransactionValidationError::TransactionSignatureMismatch {
            expected: expected.transaction_signature.clone(),
            actual: actual_signature.clone(),
        });
    }

    // Finalized but failed transaction
    if let Some(error) = &transaction.error {
        return Err(TransactionValidationError::TransactionExecutionFailed {
            error: error.clone(),
        });
    }

    let is_creator_signed = transaction
        .signer_addresses
        .iter()
        .any(|signer| signer == &expected.creator_wallet);

    if !is_creator_signed {
        return Err(TransactionValidationError::CreatorDidNotSign(
            expected.creator_wallet.clone(),
        ));
    }

    /*
     * Search only partially-decoded instructions because custom Anchor
     * instructions are returned in that representation.
     *
     * Both the program ID and discriminator must match. Another
     * instruction belonging to our program is not sufficient.
     */
    let matching_instructions: Vec<_> = transaction
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            TransactionInstruction::PartiallyDecoded {
                program_id,
                accounts,
                data,
            } => {
                if program_id == &expected.program_id
                    && data.starts_with(&INITIALIZE_ORGANIZATION_DISCRIMINATOR)
                {
                    Some((accounts.as_slice(), data.as_slice()))
                } else {
                    None
                }
            }

            _ => None,
        })
        .collect();

    let (accounts, data) = match matching_instructions.as_slice() {
        [] => {
            return Err(
                TransactionValidationError::InitializeOrganizationInstructionNotFound {
                    program_id: expected.program_id.clone(),
                },
            );
        }

        [instruction] => *instruction,

        instructions => {
            return Err(
                TransactionValidationError::MultipleInitializeOrganizationInstructions {
                    program_id: expected.program_id.clone(),
                    count: instructions.len(),
                },
            );
        }
    };

    /*
     * Anchor instruction account order is part of the instruction ABI.
     *
     * initialize_organization expects:
     *
     * 0. creator
     * 1. organization
     * 2. owner_member
     * 3. owner_member_wallet
     * 4. system_program
     */
    let expected_accounts = vec![
        expected.creator_wallet.clone(),
        expected.organization_pda.clone(),
        expected.owner_member_pda.clone(),
        expected.owner_member_wallet_pda.clone(),
        SYSTEM_PROGRAM_ID.to_owned(),
    ];

    if accounts != expected_accounts.as_slice() {
        return Err(TransactionValidationError::InstructionAccountsMismatch {
            expected: expected_accounts,
            actual: accounts.to_vec(),
        });
    }

    /*
     * Anchor instruction data layout:
     *
     * bytes  0..8  : instruction discriminator
     * bytes  8..16 : organization_id as little-endian u64
     * bytes 16..24 : owner_member_id as little-endian u64
     */
    if data.len() != 24 {
        return Err(TransactionValidationError::InvalidInstructionDataLength {
            actual: data.len(),
        });
    }

    let mut organization_id_bytes = [0_u8; 8];
    organization_id_bytes.copy_from_slice(&data[8..16]);

    let actual_organization_id = u64::from_le_bytes(organization_id_bytes);

    if actual_organization_id != expected.organization_id {
        return Err(TransactionValidationError::OrganizationIdMismatch {
            expected: expected.organization_id,
            actual: actual_organization_id,
        });
    }

    let mut owner_member_id_bytes = [0_u8; 8];
    owner_member_id_bytes.copy_from_slice(&data[16..24]);

    let actual_owner_member_id = u64::from_le_bytes(owner_member_id_bytes);

    if actual_owner_member_id != expected.owner_member_id {
        return Err(TransactionValidationError::OwnerMemberIdMismatch {
            expected: expected.owner_member_id,
            actual: actual_owner_member_id,
        });
    }

    Ok(ValidatedInitializeOrganization {
        transaction_slot: transaction.slot,
        block_time: transaction.block_time,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;

    fn expectation() -> InitializeOrganizationExpectation {
        InitializeOrganizationExpectation {
            transaction_signature: "expected-transaction-signature".to_owned(),
            program_id: "expected-program-id".to_owned(),

            creator_wallet: "creator-wallet".to_owned(),
            organization_pda: "organization-pda".to_owned(),
            owner_member_pda: "owner-member-pda".to_owned(),
            owner_member_wallet_pda: "owner-member-wallet-pda".to_owned(),

            organization_id: 42,
            owner_member_id: 7,
        }
    }

    fn initialize_instruction_data(organization_id: u64, owner_member_id: u64) -> Vec<u8> {
        let mut data = Vec::with_capacity(24);

        data.extend_from_slice(&INITIALIZE_ORGANIZATION_DISCRIMINATOR);

        data.extend_from_slice(&organization_id.to_le_bytes());

        data.extend_from_slice(&owner_member_id.to_le_bytes());

        data
    }

    fn initialize_instruction(
        expected: &InitializeOrganizationExpectation,
        organization_id: u64,
        owner_member_id: u64,
    ) -> TransactionInstruction {
        TransactionInstruction::PartiallyDecoded {
            program_id: expected.program_id.clone(),

            accounts: vec![
                expected.creator_wallet.clone(),
                expected.organization_pda.clone(),
                expected.owner_member_pda.clone(),
                expected.owner_member_wallet_pda.clone(),
                SYSTEM_PROGRAM_ID.to_owned(),
            ],

            data: initialize_instruction_data(organization_id, owner_member_id),
        }
    }

    fn valid_transaction(expected: &InitializeOrganizationExpectation) -> FinalizedTransaction {
        FinalizedTransaction {
            slot: 123_456,
            block_time: Some(1_800_000_000),
            error: None,

            signatures: vec![expected.transaction_signature.clone()],

            signer_addresses: vec![expected.creator_wallet.clone()],

            instructions: vec![initialize_instruction(
                expected,
                expected.organization_id,
                expected.owner_member_id,
            )],
        }
    }

    #[test]
    fn discriminator_matches_anchor_instruction_name() {
        let hash = Sha256::digest(b"global:initialize_organization");

        assert_eq!(&hash[..8], INITIALIZE_ORGANIZATION_DISCRIMINATOR.as_slice());
    }

    #[test]
    fn valid_initialize_organization_transaction_is_accepted() {
        let expected = expectation();
        let transaction = valid_transaction(&expected);

        let validated = validate_initialize_organization_transaction(&transaction, &expected)
            .expect("valid transaction should pass validation");

        assert_eq!(
            validated,
            ValidatedInitializeOrganization {
                transaction_slot: 123_456,
                block_time: Some(1_800_000_000),
            }
        );
    }

    #[test]
    fn wrong_transaction_signature_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.signatures[0] = "different-transaction-signature".to_owned();

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("wrong transaction signature must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::TransactionSignatureMismatch { .. }
        ));
    }

    #[test]
    fn finalized_transaction_execution_failure_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.error = Some(json!({
            "InstructionError": [0, {
                "Custom": 6000
            }]
        }));

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("failed transaction must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::TransactionExecutionFailed { .. }
        ));
    }

    #[test]
    fn missing_creator_signature_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.signer_addresses = vec!["different-wallet".to_owned()];

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("missing creator signature must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::CreatorDidNotSign(
                wallet
            ) if wallet == expected.creator_wallet
        ));
    }

    #[test]
    fn missing_initialize_instruction_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.instructions.clear();

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("missing initialization instruction must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::InitializeOrganizationInstructionNotFound { .. }
        ));
    }

    #[test]
    fn multiple_initialize_instructions_are_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.instructions.push(initialize_instruction(
            &expected,
            expected.organization_id,
            expected.owner_member_id,
        ));

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("multiple initialization instructions must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::MultipleInitializeOrganizationInstructions { count: 2, .. }
        ));
    }

    #[test]
    fn wrong_instruction_account_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        let TransactionInstruction::PartiallyDecoded { accounts, .. } =
            &mut transaction.instructions[0]
        else {
            panic!("test transaction must contain a partially decoded instruction");
        };

        accounts[1] = "wrong-organization-pda".to_owned();

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("wrong organization PDA must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::InstructionAccountsMismatch { .. }
        ));
    }

    #[test]
    fn wrong_organization_id_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.instructions = vec![initialize_instruction(
            &expected,
            999,
            expected.owner_member_id,
        )];

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("wrong organization ID must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::OrganizationIdMismatch {
                expected: 42,
                actual: 999,
            }
        ));
    }

    #[test]
    fn wrong_owner_member_id_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        transaction.instructions = vec![initialize_instruction(
            &expected,
            expected.organization_id,
            999,
        )];

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("wrong owner member ID must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::OwnerMemberIdMismatch {
                expected: 7,
                actual: 999,
            }
        ));
    }

    #[test]
    fn unexpected_instruction_data_length_is_rejected() {
        let expected = expectation();
        let mut transaction = valid_transaction(&expected);

        let TransactionInstruction::PartiallyDecoded { data, .. } =
            &mut transaction.instructions[0]
        else {
            panic!("test transaction must contain a partially decoded instruction");
        };

        /*
         * Keep the discriminator so the instruction is found, but
         * remove both serialized instruction arguments.
         */
        data.truncate(8);

        let error = validate_initialize_organization_transaction(&transaction, &expected)
            .expect_err("short instruction data must be rejected");

        assert!(matches!(
            error,
            TransactionValidationError::InvalidInstructionDataLength { actual: 8 }
        ));
    }
}
