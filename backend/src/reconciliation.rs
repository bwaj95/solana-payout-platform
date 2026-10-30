use bigdecimal::BigDecimal;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use thiserror::Error;

use crate::{
    account_validation::ValidatedInitializeOrganizationAccounts,
    domain::{
        INITIALIZE_ORGANIZATION_INSTRUCTION, RECONCILE_TRANSACTION_SUBMISSION_JOB, ROLE_OWNER,
        TransactionSubmissionStatus,
    },
    jobs::{ClaimedJob, JobQueueError, defer_job, mark_job_succeeded},
    solana_rpc::{ConfirmationStatus, SignatureStatusLookup},
};

struct ReconciliationSubmissionRow {
    submission_id: i64,
    organization_id: BigDecimal,

    cluster: String,
    program_id: String,
    instruction_name: String,
    signer_wallet: String,

    transaction_signature: Option<String>,
    last_valid_block_height: Option<i64>,
    status: String,

    owner_member_id: BigDecimal,
    organization_status: String,
    owner_member_status: String,
}

/*
 * Complete application context required to reconcile one
 * initialize_organization transaction.
 *
 * Unlike the SQL row, numeric on-chain identifiers are converted to
 * u64 before reaching the Solana validation layer.
 */
#[derive(Debug)]
pub struct InitializeOrganizationReconciliationContext {
    pub submission_id: i64,

    pub organization_id: u64,
    pub owner_member_id: u64,

    pub cluster: String,
    pub program_id: String,
    pub signer_wallet: String,

    pub transaction_signature: String,
    pub last_valid_block_height: u64,
    pub submission_status: TransactionSubmissionStatus,

    pub organization_status: String,
    pub owner_member_status: String,
}

#[derive(Debug, Error)]
pub enum ReconciliationLoadError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error("job {job_id} has type {actual}, expected reconcile_transaction_submission")]
    WrongJobType { job_id: i64, actual: String },

    #[error("job {job_id} has no submission ID")]
    MissingSubmissionId { job_id: i64 },

    #[error("job {job_id} has no organization ID")]
    MissingOrganizationId { job_id: i64 },

    #[error("submission {submission_id} was not found")]
    SubmissionNotFound { submission_id: i64 },

    #[error(
        "job organization {job_organization_id} does not match submission organization {submission_organization_id}"
    )]
    OrganizationMismatch {
        job_organization_id: String,
        submission_organization_id: String,
    },

    #[error(
        "submission {submission_id} targets cluster {actual}, but this worker handles {expected}"
    )]
    ClusterMismatch {
        submission_id: i64,
        expected: String,
        actual: String,
    },

    #[error(
        "submission {submission_id} targets program {actual}, but this worker handles {expected}"
    )]
    ProgramMismatch {
        submission_id: i64,
        expected: String,
        actual: String,
    },

    #[error("submission {submission_id} contains unsupported instruction {instruction_name}")]
    UnsupportedInstruction {
        submission_id: i64,
        instruction_name: String,
    },

    #[error("submission {submission_id} has no transaction signature")]
    MissingTransactionSignature { submission_id: i64 },

    #[error("submission {submission_id} has no last valid block height")]
    MissingLastValidBlockHeight { submission_id: i64 },

    #[error("submission {submission_id} has invalid negative last valid block height {value}")]
    InvalidLastValidBlockHeight { submission_id: i64, value: i64 },

    #[error("submission {submission_id} has unsupported status {status}")]
    InvalidSubmissionStatus { submission_id: i64, status: String },

    #[error("{field} value {value} cannot be represented as u64")]
    InvalidU64 { field: &'static str, value: String },
}

#[derive(Debug, Error)]
pub enum ReconciliationSettlementError {
    #[error(transparent)]
    Database(#[from] sqlx::Error),

    #[error(transparent)]
    JobQueue(#[from] JobQueueError),

    #[error(
        "claimed job {job_id} references submission {claimed_submission_id:?}, \
         but reconciliation tried to settle submission {settled_submission_id}"
    )]
    SubmissionMismatch {
        job_id: i64,
        claimed_submission_id: Option<i64>,
        settled_submission_id: i64,
    },

    #[error("submission {submission_id} cannot record non-terminal status {status}")]
    InvalidNonTerminalStatus {
        submission_id: i64,
        status: &'static str,
    },

    #[error(
        "confirmed submission observation for submission {submission_id} \
         did not contain a transaction slot"
    )]
    MissingConfirmedSlot { submission_id: i64 },

    #[error("Solana slot {slot} cannot be represented by PostgreSQL BIGINT")]
    SlotOutOfRange { slot: u64 },

    #[error(
        "submission {submission_id} was missing or was no longer in a \
         reconcilable non-terminal state"
    )]
    SubmissionNotReconcilable { submission_id: i64 },

    #[error(
        "organization {organization_id} was missing or could not be restored \
     from provisioning to draft"
    )]
    OrganizationNotResettable { organization_id: u64 },

    #[error(
        "owner member {member_id} of organization {organization_id} was \
     missing or could not be restored from provisioning to draft"
    )]
    OwnerMemberNotResettable {
        organization_id: u64,
        member_id: u64,
    },

    #[error(
        "validated account field {field} mismatch: expected {expected}, \
     received {actual}"
    )]
    ValidatedAccountMismatch {
        field: &'static str,
        expected: String,
        actual: String,
    },

    #[error(
        "account snapshot slot {account_slot} precedes transaction slot \
     {transaction_slot}"
    )]
    AccountSnapshotPrecedesTransaction {
        transaction_slot: u64,
        account_slot: u64,
    },

    #[error("organization {organization_id} was missing or could not be activated")]
    OrganizationNotActivatable { organization_id: u64 },

    #[error(
        "owner member {member_id} of organization {organization_id} was \
     missing or could not be activated"
    )]
    OwnerMemberNotActivatable {
        organization_id: u64,
        member_id: u64,
    },
}

/*
 * Load the context without FOR UPDATE.
 *
 * This read happens before network calls. Holding a PostgreSQL row
 * lock while waiting for Solana RPC would create a long transaction.
 *
 * The settlement transaction will lock and revalidate the submission
 * again immediately before performing database writes.
 */
pub async fn load_initialize_organization_context(
    connection: &mut PgConnection,
    claimed_job: &ClaimedJob,
    worker_cluster: &str,
    worker_program_id: &str,
) -> Result<InitializeOrganizationReconciliationContext, ReconciliationLoadError> {
    if claimed_job.job_type != RECONCILE_TRANSACTION_SUBMISSION_JOB {
        return Err(ReconciliationLoadError::WrongJobType {
            job_id: claimed_job.job_id,
            actual: claimed_job.job_type.clone(),
        });
    }

    let submission_id =
        claimed_job
            .submission_id
            .ok_or(ReconciliationLoadError::MissingSubmissionId {
                job_id: claimed_job.job_id,
            })?;

    let job_organization_id = claimed_job.organization_id.as_ref().ok_or(
        ReconciliationLoadError::MissingOrganizationId {
            job_id: claimed_job.job_id,
        },
    )?;

    let row = sqlx::query_as!(
        ReconciliationSubmissionRow,
        r#"
        SELECT
            submission.submission_id,
            submission.organization_id,

            submission.cluster,
            submission.program_id,
            submission.instruction_name,
            submission.signer_wallet,

            submission.transaction_signature,
            submission.last_valid_block_height,
            submission.status,

            owner_member.member_id AS owner_member_id,
            organization.status AS organization_status,
            owner_member.lifecycle_status
                AS owner_member_status

        FROM transaction_submissions AS submission

        JOIN organizations AS organization
          ON organization.organization_id =
             submission.organization_id

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             submission.organization_id
         AND (owner_member.requested_roles & $2) <> 0

        WHERE submission.submission_id = $1
        "#,
        submission_id,
        ROLE_OWNER,
    )
    .fetch_optional(connection)
    .await?
    .ok_or(ReconciliationLoadError::SubmissionNotFound { submission_id })?;

    if &row.organization_id != job_organization_id {
        return Err(ReconciliationLoadError::OrganizationMismatch {
            job_organization_id: job_organization_id.to_string(),
            submission_organization_id: row.organization_id.to_string(),
        });
    }

    if row.cluster != worker_cluster {
        return Err(ReconciliationLoadError::ClusterMismatch {
            submission_id,
            expected: worker_cluster.to_owned(),
            actual: row.cluster,
        });
    }

    if row.program_id != worker_program_id {
        return Err(ReconciliationLoadError::ProgramMismatch {
            submission_id,
            expected: worker_program_id.to_owned(),
            actual: row.program_id,
        });
    }

    if row.instruction_name != INITIALIZE_ORGANIZATION_INSTRUCTION {
        return Err(ReconciliationLoadError::UnsupportedInstruction {
            submission_id,
            instruction_name: row.instruction_name,
        });
    }

    let organization_id = decimal_to_u64(&row.organization_id, "organization_id")?;

    let owner_member_id = decimal_to_u64(&row.owner_member_id, "owner_member_id")?;

    let transaction_signature = row
        .transaction_signature
        .ok_or(ReconciliationLoadError::MissingTransactionSignature { submission_id })?;

    let last_valid_block_height_i64 = row
        .last_valid_block_height
        .ok_or(ReconciliationLoadError::MissingLastValidBlockHeight { submission_id })?;

    let last_valid_block_height = u64::try_from(last_valid_block_height_i64).map_err(|_| {
        ReconciliationLoadError::InvalidLastValidBlockHeight {
            submission_id,
            value: last_valid_block_height_i64,
        }
    })?;

    let submission_status = parse_submission_status(submission_id, &row.status)?;

    Ok(InitializeOrganizationReconciliationContext {
        submission_id,

        organization_id,
        owner_member_id,

        cluster: row.cluster,
        program_id: row.program_id,
        signer_wallet: row.signer_wallet,

        transaction_signature,
        last_valid_block_height,
        submission_status,

        organization_status: row.organization_status,
        owner_member_status: row.owner_member_status,
    })
}

pub async fn settle_deferred_signature_observation(
    pool: &PgPool,
    claimed_job: &ClaimedJob,
    submission_id: i64,
    observed_status: TransactionSubmissionStatus,
    confirmed_transaction_slot: Option<u64>,
    defer_seconds: i64,
) -> Result<(), ReconciliationSettlementError> {
    /*
     * The submission ID obtained from the reconciliation context must still
     * agree with the submission attached to the claimed job.
     *
     * This prevents a programming mistake from updating one submission while
     * releasing the lease for a different job.
     */
    if claimed_job.submission_id != Some(submission_id) {
        return Err(ReconciliationSettlementError::SubmissionMismatch {
            job_id: claimed_job.job_id,
            claimed_submission_id: claimed_job.submission_id,
            settled_submission_id: submission_id,
        });
    }

    /*
     * This function is only for non-terminal observations.
     *
     * submitted:
     *   The RPC found the transaction at processed commitment.
     *
     * unknown:
     *   The RPC did not currently find the signature, but its blockhash has
     *   not expired.
     *
     * confirmed:
     *   The transaction reached confirmed commitment but is not finalized.
     */
    if !matches!(
        observed_status,
        TransactionSubmissionStatus::Submitted
            | TransactionSubmissionStatus::Unknown
            | TransactionSubmissionStatus::Confirmed
    ) {
        return Err(ReconciliationSettlementError::InvalidNonTerminalStatus {
            submission_id,
            status: observed_status.as_str(),
        });
    }

    /*
     * A confirmed observation must tell us the transaction's slot.
     *
     * Processed and missing-signature observations do not populate the
     * confirmed_slot column.
     */
    let confirmed_slot = match observed_status {
        TransactionSubmissionStatus::Confirmed => {
            let slot = confirmed_transaction_slot
                .ok_or(ReconciliationSettlementError::MissingConfirmedSlot { submission_id })?;

            Some(
                i64::try_from(slot)
                    .map_err(|_| ReconciliationSettlementError::SlotOutOfRange { slot })?,
            )
        }

        _ => None,
    };

    let observed_status = observed_status.as_str();

    /*
     * Both writes belong to one PostgreSQL transaction:
     *
     * 1. Record what the RPC observed.
     * 2. Return the job to the queue and release its logical lease.
     *
     * If defer_job detects that this worker lost its lease, it returns an
     * error. The transaction is then dropped without commit, so the submission
     * update is rolled back as well.
     */
    let mut transaction = pool.begin().await?;

    let updated_submission_status = sqlx::query_scalar!(
        r#"
        UPDATE transaction_submissions

        SET
            /*
             * A confirmed submission must never move backwards because a
             * later request reached a stale RPC node.
             *
             * submitted and unknown may move between each other because they
             * represent the latest ambiguous, non-final observation.
             */
            status = CASE
                WHEN status = 'confirmed' THEN 'confirmed'
                ELSE $2::TEXT
            END,

            confirmed_slot = CASE
                WHEN status = 'confirmed' THEN confirmed_slot
                WHEN $2::TEXT = 'confirmed' THEN $3::BIGINT
                ELSE confirmed_slot
            END,

            observed_commitment = CASE
                WHEN status = 'confirmed' THEN observed_commitment
                WHEN $2::TEXT = 'confirmed' THEN 'confirmed'
                ELSE observed_commitment
            END,

            confirmed_at = CASE
                WHEN status = 'confirmed' THEN confirmed_at
                WHEN $2::TEXT = 'confirmed'
                    THEN COALESCE(confirmed_at, now())
                ELSE confirmed_at
            END,

            last_checked_at = now(),

            /*
             * A valid non-terminal RPC observation means there is no current
             * terminal transaction error to expose.
             */
            error_code = NULL,
            error_message = NULL

        WHERE submission_id = $1
          AND status IN (
              'submitted',
              'unknown',
              'confirmed'
          )

        RETURNING status
        "#,
        submission_id,
        observed_status,
        confirmed_slot,
    )
    .fetch_optional(&mut *transaction)
    .await?;

    if updated_submission_status.is_none() {
        return Err(ReconciliationSettlementError::SubmissionNotReconcilable { submission_id });
    }

    /*
     * defer_job performs the fencing check:
     *
     * - job is still running;
     * - attempt still matches;
     * - locked_by still identifies this worker;
     * - lease has not expired.
     *
     * It then requeues the job and clears its lease fields.
     */
    defer_job(&mut *transaction, claimed_job, defer_seconds).await?;

    transaction.commit().await?;

    Ok(())
}

pub async fn settle_terminal_submission_failure(
    pool: &PgPool,
    claimed_job: &ClaimedJob,
    context: &InitializeOrganizationReconciliationContext,
    outcome: &TerminalSubmissionOutcome,
) -> Result<(), ReconciliationSettlementError> {
    /*
     * Protect against accidentally settling a different submission from the
     * one attached to the claimed job.
     */
    if claimed_job.submission_id != Some(context.submission_id) {
        return Err(ReconciliationSettlementError::SubmissionMismatch {
            job_id: claimed_job.job_id,
            claimed_submission_id: claimed_job.submission_id,
            settled_submission_id: context.submission_id,
        });
    }

    /*
     * Translate the domain outcome into database values.
     *
     * A finalized failed transaction has a real transaction slot and finalized
     * commitment.
     *
     * An expired transaction was never found in a terminal chain state, so it
     * has no finalized transaction slot.
     */
    let (terminal_status, transaction_slot, is_finalized_failure, error_code, error_message) =
        match outcome {
            TerminalSubmissionOutcome::Expired => (
                TransactionSubmissionStatus::Expired.as_str(),
                None,
                false,
                "transaction_expired",
                format!(
                    "Transaction was not found after last valid block height {}.",
                    context.last_valid_block_height,
                ),
            ),

            TerminalSubmissionOutcome::FinalizedFailure {
                transaction_slot,
                error,
            } => {
                let transaction_slot = i64::try_from(*transaction_slot).map_err(|_| {
                    ReconciliationSettlementError::SlotOutOfRange {
                        slot: *transaction_slot,
                    }
                })?;

                (
                    TransactionSubmissionStatus::Failed.as_str(),
                    Some(transaction_slot),
                    true,
                    "solana_transaction_failed",
                    error.to_string(),
                )
            }
        };

    let organization_id = BigDecimal::from(context.organization_id);

    let owner_member_id = BigDecimal::from(context.owner_member_id);

    /*
     * This transaction is the complete durable settlement:
     *
     * 1. Record the terminal submission outcome.
     * 2. Restore application lifecycle rows to draft.
     * 3. Mark the reconciliation job successful.
     *
     * Any error—including a lost job lease—rolls back every write.
     */
    let mut transaction = pool.begin().await?;

    let updated_submission_id = sqlx::query_scalar!(
        r#"
        UPDATE transaction_submissions

        SET
            status = $2,

            confirmed_slot = CASE
                WHEN $3::BOOLEAN THEN $4::BIGINT
                ELSE confirmed_slot
            END,

            observed_commitment = CASE
                WHEN $3::BOOLEAN THEN 'finalized'
                ELSE observed_commitment
            END,

            confirmed_at = CASE
                WHEN $3::BOOLEAN
                    THEN COALESCE(confirmed_at, now())
                ELSE confirmed_at
            END,

            finalized_at = CASE
                WHEN $3::BOOLEAN THEN now()
                ELSE finalized_at
            END,

            failed_at = now(),
            last_checked_at = now(),

            error_code = $5,
            error_message = $6

        WHERE submission_id = $1
          AND status IN (
              'submitted',
              'unknown',
              'confirmed'
          )

        RETURNING submission_id
        "#,
        context.submission_id,
        terminal_status,
        is_finalized_failure,
        transaction_slot,
        error_code,
        &error_message,
    )
    .fetch_optional(&mut *transaction)
    .await?;

    if updated_submission_id.is_none() {
        return Err(ReconciliationSettlementError::SubmissionNotReconcilable {
            submission_id: context.submission_id,
        });
    }

    /*
     * Initialization did not succeed, so the application records return to
     * draft and can be submitted again with a new Solana transaction.
     *
     * Restricting the current state to draft/provisioning prevents an old
     * worker from accidentally moving an already-active organization
     * backwards.
     */
    let organization_update = sqlx::query!(
        r#"
        UPDATE organizations
        SET status = 'draft'
        WHERE organization_id = $1
          AND status IN (
              'draft',
              'provisioning'
          )
        "#,
        &organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    if organization_update.rows_affected() != 1 {
        return Err(ReconciliationSettlementError::OrganizationNotResettable {
            organization_id: context.organization_id,
        });
    }

    let owner_member_update = sqlx::query!(
        r#"
        UPDATE organization_members
        SET lifecycle_status = 'draft'
        WHERE organization_id = $1
          AND member_id = $2
          AND lifecycle_status IN (
              'draft',
              'provisioning'
          )
        "#,
        &organization_id,
        &owner_member_id,
    )
    .execute(&mut *transaction)
    .await?;

    if owner_member_update.rows_affected() != 1 {
        return Err(ReconciliationSettlementError::OwnerMemberNotResettable {
            organization_id: context.organization_id,
            member_id: context.owner_member_id,
        });
    }

    /*
     * The job itself succeeded: it correctly discovered and persisted the
     * terminal Solana outcome.
     *
     * mark_job_succeeded also performs the lease/attempt fencing check.
     * If the worker became stale, this fails and all preceding changes roll
     * back.
     */
    mark_job_succeeded(&mut *transaction, claimed_job).await?;

    transaction.commit().await?;

    Ok(())
}

pub async fn settle_finalized_initialize_organization(
    pool: &PgPool,
    claimed_job: &ClaimedJob,
    context: &InitializeOrganizationReconciliationContext,
    transaction_slot: u64,
    accounts: &ValidatedInitializeOrganizationAccounts,
) -> Result<(), ReconciliationSettlementError> {
    /*
     * Protect against accidentally completing one job with another
     * submission's reconciliation result.
     */
    if claimed_job.submission_id != Some(context.submission_id) {
        return Err(ReconciliationSettlementError::SubmissionMismatch {
            job_id: claimed_job.job_id,
            claimed_submission_id: claimed_job.submission_id,
            settled_submission_id: context.submission_id,
        });
    }

    /*
     * Account validation should already have checked these relationships.
     *
     * Rechecking the core identities here protects this persistence boundary
     * from a future caller accidentally supplying validated accounts belonging
     * to another reconciliation context.
     */
    if accounts.organization.organization_id != context.organization_id {
        return Err(ReconciliationSettlementError::ValidatedAccountMismatch {
            field: "organization.organization_id",
            expected: context.organization_id.to_string(),
            actual: accounts.organization.organization_id.to_string(),
        });
    }

    if accounts.owner_member.member_id != context.owner_member_id {
        return Err(ReconciliationSettlementError::ValidatedAccountMismatch {
            field: "owner_member.member_id",
            expected: context.owner_member_id.to_string(),
            actual: accounts.owner_member.member_id.to_string(),
        });
    }

    if accounts.organization.creator_wallet != context.signer_wallet {
        return Err(ReconciliationSettlementError::ValidatedAccountMismatch {
            field: "organization.creator_wallet",
            expected: context.signer_wallet.clone(),
            actual: accounts.organization.creator_wallet.clone(),
        });
    }

    if accounts.owner_member.organization_pda != accounts.organization.address {
        return Err(ReconciliationSettlementError::ValidatedAccountMismatch {
            field: "owner_member.organization_pda",
            expected: accounts.organization.address.clone(),
            actual: accounts.owner_member.organization_pda.clone(),
        });
    }

    if accounts.owner_member_wallet.organization_pda != accounts.organization.address {
        return Err(ReconciliationSettlementError::ValidatedAccountMismatch {
            field: "owner_member_wallet.organization_pda",
            expected: accounts.organization.address.clone(),
            actual: accounts.owner_member_wallet.organization_pda.clone(),
        });
    }

    if accounts.owner_member_wallet.member_pda != accounts.owner_member.address {
        return Err(ReconciliationSettlementError::ValidatedAccountMismatch {
            field: "owner_member_wallet.member_pda",
            expected: accounts.owner_member.address.clone(),
            actual: accounts.owner_member_wallet.member_pda.clone(),
        });
    }

    /*
     * The account query used minContextSlot = transaction_slot.
     *
     * Retain this check at the persistence boundary so an account snapshot
     * from before the initialization transaction can never be stored.
     */
    if accounts.observed_slot < transaction_slot {
        return Err(
            ReconciliationSettlementError::AccountSnapshotPrecedesTransaction {
                transaction_slot,
                account_slot: accounts.observed_slot,
            },
        );
    }

    let transaction_slot_i64 = i64::try_from(transaction_slot).map_err(|_| {
        ReconciliationSettlementError::SlotOutOfRange {
            slot: transaction_slot,
        }
    })?;

    let observed_slot_i64 = i64::try_from(accounts.observed_slot).map_err(|_| {
        ReconciliationSettlementError::SlotOutOfRange {
            slot: accounts.observed_slot,
        }
    })?;

    let organization_id = BigDecimal::from(context.organization_id);

    let owner_member_id = BigDecimal::from(context.owner_member_id);

    let authorization_revision = BigDecimal::from(accounts.owner_member.authorization_revision);

    let member_roles = i32::from(accounts.owner_member.roles);

    let organization_bump = i16::from(accounts.organization.bump);

    let owner_member_bump = i16::from(accounts.owner_member.bump);

    let owner_member_wallet_bump = i16::from(accounts.owner_member_wallet.bump);

    /*
     * Everything below is one short PostgreSQL transaction.
     *
     * No Solana RPC request happens while this transaction is open.
     */
    let mut transaction = pool.begin().await?;

    /*
     * Revalidate that the submission is still non-terminal.
     *
     * This also obtains the submission row lock before the projection and
     * lifecycle writes.
     */
    let finalized_submission_id = sqlx::query_scalar!(
        r#"
        UPDATE transaction_submissions

        SET
            status = 'finalized',
            confirmed_slot = $2,
            observed_commitment = 'finalized',

            last_checked_at = now(),
            confirmed_at = COALESCE(confirmed_at, now()),
            finalized_at = now(),

            failed_at = NULL,
            error_code = NULL,
            error_message = NULL

        WHERE submission_id = $1
          AND status IN (
              'submitted',
              'unknown',
              'confirmed'
          )

        RETURNING submission_id
        "#,
        context.submission_id,
        transaction_slot_i64,
    )
    .fetch_optional(&mut *transaction)
    .await?;

    if finalized_submission_id.is_none() {
        return Err(ReconciliationSettlementError::SubmissionNotReconcilable {
            submission_id: context.submission_id,
        });
    }

    /*
     * Projection 1: Organization.
     *
     * first_observed_at is intentionally not updated during a conflict.
     * last_synced_at records the latest successful synchronization.
     *
     * The slot guard prevents an older delayed observation from overwriting a
     * newer projection.
     */
    sqlx::query!(
        r#"
        INSERT INTO organization_chain_accounts (
            organization_id,
            cluster,
            program_id,
            organization_pda,
            creator_wallet,
            owner_member_pda,
            paused,
            bump,
            observed_slot,
            observed_commitment
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            'finalized'
        )

        ON CONFLICT ON CONSTRAINT organization_chain_accounts_pkey
        DO UPDATE SET
            organization_pda = EXCLUDED.organization_pda,
            creator_wallet = EXCLUDED.creator_wallet,
            owner_member_pda = EXCLUDED.owner_member_pda,
            paused = EXCLUDED.paused,
            bump = EXCLUDED.bump,
            observed_slot = EXCLUDED.observed_slot,
            observed_commitment = EXCLUDED.observed_commitment,
            last_synced_at = now()

        WHERE EXCLUDED.observed_slot
              >= organization_chain_accounts.observed_slot
        "#,
        &organization_id,
        &context.cluster,
        &context.program_id,
        &accounts.organization.address,
        &accounts.organization.creator_wallet,
        &accounts.organization.owner_member_pda,
        accounts.organization.paused,
        organization_bump,
        observed_slot_i64,
    )
    .execute(&mut *transaction)
    .await?;

    /*
     * Projection 2: owner Member.
     *
     * This must run after the Organization projection because its foreign key
     * references the exact Organization chain identity.
     */
    sqlx::query!(
        r#"
        INSERT INTO member_chain_accounts (
            organization_id,
            member_id,
            cluster,
            program_id,
            organization_pda,
            member_pda,
            authorized_wallet,
            authorization_revision,
            roles,
            active,
            bump,
            observed_slot,
            observed_commitment
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            $12,
            'finalized'
        )

        ON CONFLICT ON CONSTRAINT member_chain_accounts_pkey
        DO UPDATE SET
            organization_pda = EXCLUDED.organization_pda,
            member_pda = EXCLUDED.member_pda,
            authorized_wallet = EXCLUDED.authorized_wallet,
            authorization_revision =
                EXCLUDED.authorization_revision,
            roles = EXCLUDED.roles,
            active = EXCLUDED.active,
            bump = EXCLUDED.bump,
            observed_slot = EXCLUDED.observed_slot,
            observed_commitment =
                EXCLUDED.observed_commitment,
            last_synced_at = now()

        WHERE EXCLUDED.observed_slot
              >= member_chain_accounts.observed_slot
        "#,
        &organization_id,
        &owner_member_id,
        &context.cluster,
        &context.program_id,
        &accounts.organization.address,
        &accounts.owner_member.address,
        &accounts.owner_member.authorized_wallet,
        &authorization_revision,
        member_roles,
        accounts.owner_member.active,
        owner_member_bump,
        observed_slot_i64,
    )
    .execute(&mut *transaction)
    .await?;

    /*
     * Projection 3: MemberWallet.
     *
     * This must run last because its foreign key references the exact Member
     * chain projection written above.
     */
    sqlx::query!(
        r#"
        INSERT INTO member_wallet_chain_accounts (
            organization_id,
            member_id,
            cluster,
            program_id,
            organization_pda,
            member_pda,
            member_wallet_pda,
            authorized_wallet,
            bump,
            observed_slot,
            observed_commitment
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            'finalized'
        )

        ON CONFLICT ON CONSTRAINT member_wallet_chain_accounts_pkey
        DO UPDATE SET
            organization_id = EXCLUDED.organization_id,
            member_id = EXCLUDED.member_id,
            organization_pda = EXCLUDED.organization_pda,
            member_pda = EXCLUDED.member_pda,
            authorized_wallet = EXCLUDED.authorized_wallet,
            bump = EXCLUDED.bump,
            observed_slot = EXCLUDED.observed_slot,
            observed_commitment =
                EXCLUDED.observed_commitment,
            last_synced_at = now()

        WHERE EXCLUDED.observed_slot
              >= member_wallet_chain_accounts.observed_slot
        "#,
        &organization_id,
        &owner_member_id,
        &context.cluster,
        &context.program_id,
        &accounts.organization.address,
        &accounts.owner_member.address,
        &accounts.owner_member_wallet.address,
        &accounts.owner_member_wallet.authorized_wallet,
        owner_member_wallet_bump,
        observed_slot_i64,
    )
    .execute(&mut *transaction)
    .await?;

    /*
     * The authoritative on-chain accounts now exist and have been validated.
     * The application lifecycle can move from provisioning to active.
     *
     * Active is accepted as well so a harmless idempotent settlement does not
     * move the lifecycle backwards.
     */
    let organization_update = sqlx::query!(
        r#"
        UPDATE organizations
        SET status = 'active'
        WHERE organization_id = $1
          AND status IN (
              'provisioning',
              'active'
          )
        "#,
        &organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    if organization_update.rows_affected() != 1 {
        return Err(ReconciliationSettlementError::OrganizationNotActivatable {
            organization_id: context.organization_id,
        });
    }

    let owner_member_update = sqlx::query!(
        r#"
        UPDATE organization_members
        SET lifecycle_status = 'active'
        WHERE organization_id = $1
          AND member_id = $2
          AND lifecycle_status IN (
              'provisioning',
              'active'
          )
        "#,
        &organization_id,
        &owner_member_id,
    )
    .execute(&mut *transaction)
    .await?;

    if owner_member_update.rows_affected() != 1 {
        return Err(ReconciliationSettlementError::OwnerMemberNotActivatable {
            organization_id: context.organization_id,
            member_id: context.owner_member_id,
        });
    }

    /*
     * Fencing happens last.
     *
     * If this worker lost its lease at any time while performing RPC work,
     * mark_job_succeeded matches no row and returns LeaseLost.
     *
     * Because every write above belongs to this transaction, PostgreSQL then
     * rolls all of them back.
     */
    mark_job_succeeded(&mut *transaction, claimed_job).await?;

    transaction.commit().await?;

    Ok(())
}

fn decimal_to_u64(value: &BigDecimal, field: &'static str) -> Result<u64, ReconciliationLoadError> {
    let value_string = value.to_string();

    value_string
        .parse::<u64>()
        .map_err(|_| ReconciliationLoadError::InvalidU64 {
            field,
            value: value_string,
        })
}

fn parse_submission_status(
    submission_id: i64,
    status: &str,
) -> Result<TransactionSubmissionStatus, ReconciliationLoadError> {
    let parsed = match status {
        "prepared" => TransactionSubmissionStatus::Prepared,

        "submitted" => TransactionSubmissionStatus::Submitted,

        "unknown" => TransactionSubmissionStatus::Unknown,

        "confirmed" => TransactionSubmissionStatus::Confirmed,

        "finalized" => TransactionSubmissionStatus::Finalized,

        "failed" => TransactionSubmissionStatus::Failed,

        "expired" => TransactionSubmissionStatus::Expired,

        _ => {
            return Err(ReconciliationLoadError::InvalidSubmissionStatus {
                submission_id,
                status: status.to_owned(),
            });
        }
    };

    Ok(parsed)
}

/*
 * Explicit state-machine decision made from one RPC observation.
 *
 * Defer means the external operation is nonterminal and should be
 * checked again later without recording a processing failure.
 *
 * InspectFinalizedTransaction means signature status claims success;
 * the worker must still fetch and validate the complete transaction.
 *
 * FinalizedFailure and Expired are terminal chain outcomes. The job
 * itself can finish successfully after persisting that outcome.
 */
#[derive(Debug, Clone, PartialEq)]
pub enum SignatureReconciliationAction {
    Defer {
        submission_status: TransactionSubmissionStatus,
    },

    InspectFinalizedTransaction {
        transaction_slot: u64,
    },

    FinalizedFailure {
        transaction_slot: u64,
        error: Value,
    },

    Expired,
}

/*
 * A terminal unsuccessful Solana outcome.
 *
 * Expired means the signature was not found after its last valid block
 * height.
 *
 * FinalizedFailure means Solana finalized the transaction, but program
 * execution returned an error.
 */
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalSubmissionOutcome {
    Expired,

    FinalizedFailure { transaction_slot: u64, error: Value },
}

#[derive(Debug, Error)]
pub enum SignatureClassificationError {
    #[error("current finalized block height is required when the signature is missing")]
    MissingCurrentBlockHeight,
}

pub fn classify_signature_lookup(
    lookup: &SignatureStatusLookup,
    last_valid_block_height: u64,

    /*
     * The caller only needs to make getBlockHeight when the
     * signature lookup returned None.
     */
    current_finalized_block_height: Option<u64>,
) -> Result<SignatureReconciliationAction, SignatureClassificationError> {
    let Some(found_status) = &lookup.found_status else {
        let current_block_height = current_finalized_block_height
            .ok_or(SignatureClassificationError::MissingCurrentBlockHeight)?;

        /*
         * The transaction remains valid through its last valid block
         * height. It is expired only after that height is exceeded.
         */
        if current_block_height > last_valid_block_height {
            return Ok(SignatureReconciliationAction::Expired);
        }

        return Ok(SignatureReconciliationAction::Defer {
            submission_status: TransactionSubmissionStatus::Unknown,
        });
    };

    match found_status.confirmation_status {
        ConfirmationStatus::Processed => Ok(SignatureReconciliationAction::Defer {
            submission_status: TransactionSubmissionStatus::Submitted,
        }),

        ConfirmationStatus::Confirmed => Ok(SignatureReconciliationAction::Defer {
            submission_status: TransactionSubmissionStatus::Confirmed,
        }),

        ConfirmationStatus::Finalized => match &found_status.error {
            Some(error) => Ok(SignatureReconciliationAction::FinalizedFailure {
                transaction_slot: found_status.transaction_slot,
                error: error.clone(),
            }),

            None => Ok(SignatureReconciliationAction::InspectFinalizedTransaction {
                transaction_slot: found_status.transaction_slot,
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::solana_rpc::FoundSignatureStatus;

    fn lookup(found_status: Option<FoundSignatureStatus>) -> SignatureStatusLookup {
        SignatureStatusLookup {
            context_slot: 1_000,
            found_status,
        }
    }

    fn found_status(
        confirmation_status: ConfirmationStatus,
        error: Option<Value>,
    ) -> FoundSignatureStatus {
        FoundSignatureStatus {
            transaction_slot: 900,
            confirmation_status,
            error,
        }
    }

    #[test]
    fn missing_signature_requires_current_block_height() {
        let result = classify_signature_lookup(&lookup(None), 500, None);

        assert!(matches!(
            result,
            Err(SignatureClassificationError::MissingCurrentBlockHeight)
        ));
    }

    #[test]
    fn missing_signature_before_expiration_is_deferred_as_unknown() {
        let action = classify_signature_lookup(&lookup(None), 500, Some(499))
            .expect("classification should succeed");

        assert_eq!(
            action,
            SignatureReconciliationAction::Defer {
                submission_status: TransactionSubmissionStatus::Unknown,
            }
        );
    }

    #[test]
    fn missing_signature_at_last_valid_block_height_is_not_expired() {
        let action = classify_signature_lookup(&lookup(None), 500, Some(500))
            .expect("classification should succeed");

        assert_eq!(
            action,
            SignatureReconciliationAction::Defer {
                submission_status: TransactionSubmissionStatus::Unknown,
            }
        );
    }

    #[test]
    fn missing_signature_after_last_valid_block_height_is_expired() {
        let action = classify_signature_lookup(&lookup(None), 500, Some(501))
            .expect("classification should succeed");

        assert_eq!(action, SignatureReconciliationAction::Expired);
    }

    #[test]
    fn processed_signature_is_deferred_as_submitted() {
        let action = classify_signature_lookup(
            &lookup(Some(found_status(ConfirmationStatus::Processed, None))),
            500,
            None,
        )
        .expect("classification should succeed");

        assert_eq!(
            action,
            SignatureReconciliationAction::Defer {
                submission_status: TransactionSubmissionStatus::Submitted,
            }
        );
    }

    #[test]
    fn confirmed_signature_is_deferred_as_confirmed() {
        let action = classify_signature_lookup(
            &lookup(Some(found_status(ConfirmationStatus::Confirmed, None))),
            500,
            None,
        )
        .expect("classification should succeed");

        assert_eq!(
            action,
            SignatureReconciliationAction::Defer {
                submission_status: TransactionSubmissionStatus::Confirmed,
            }
        );
    }

    #[test]
    fn finalized_success_is_ready_for_transaction_inspection() {
        let action = classify_signature_lookup(
            &lookup(Some(found_status(ConfirmationStatus::Finalized, None))),
            500,
            None,
        )
        .expect("classification should succeed");

        assert_eq!(
            action,
            SignatureReconciliationAction::InspectFinalizedTransaction {
                transaction_slot: 900,
            }
        );
    }

    #[test]
    fn finalized_transaction_error_is_terminal_failure() {
        let transaction_error = json!({
            "InstructionError": [
                2,
                {
                    "Custom": 6001
                }
            ]
        });

        let action = classify_signature_lookup(
            &lookup(Some(found_status(
                ConfirmationStatus::Finalized,
                Some(transaction_error.clone()),
            ))),
            500,
            None,
        )
        .expect("classification should succeed");

        assert_eq!(
            action,
            SignatureReconciliationAction::FinalizedFailure {
                transaction_slot: 900,
                error: transaction_error,
            }
        );
    }
}
