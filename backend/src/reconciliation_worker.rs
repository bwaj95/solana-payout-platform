use sqlx::PgPool;
use thiserror::Error;

use crate::{
    account_validation::{
        AccountValidationError, InitializeOrganizationAccountsExpectation,
        validate_initialize_organization_accounts,
    },
    domain::TransactionSubmissionStatus,
    jobs::{ClaimedJob, JobQueueError},
    pdas::{PdaDerivationError, derive_initialize_organization_pdas},
    reconciliation::{
        InitializeOrganizationReconciliationContext, ReconciliationLoadError,
        ReconciliationSettlementError, SignatureClassificationError, SignatureReconciliationAction,
        TerminalSubmissionOutcome, classify_signature_lookup, load_initialize_organization_context,
        settle_deferred_signature_observation, settle_finalized_initialize_organization,
        settle_terminal_submission_failure,
    },
    solana_rpc::{
        FinalizedAccountsLookup, FinalizedTransaction, SignatureStatusLookup, SolanaRpcClient,
        SolanaRpcError,
    },
    transaction_validation::{
        InitializeOrganizationExpectation, TransactionValidationError,
        validate_initialize_organization_transaction,
    },
};

/*
 * The minimum Solana RPC behavior needed by this processor.
 *
 * Production supplies SolanaRpcClient.
 * Tests can supply a deterministic fake without making network calls.
 *
 * The processor depends on this behavior contract rather than on HTTP
 * implementation details.
 */
#[allow(async_fn_in_trait)]
pub trait ReconciliationRpc: Send + Sync {
    async fn get_signature_status(
        &self,
        signature: &str,
    ) -> Result<SignatureStatusLookup, SolanaRpcError>;

    async fn get_finalized_block_height(&self) -> Result<u64, SolanaRpcError>;

    async fn get_finalized_transaction(
        &self,
        signature: &str,
    ) -> Result<Option<FinalizedTransaction>, SolanaRpcError>;

    async fn get_finalized_accounts(
        &self,
        addresses: &[String],
        min_context_slot: u64,
    ) -> Result<FinalizedAccountsLookup, SolanaRpcError>;
}

/*
 * Production adapter.
 *
 * Each method delegates to the real HTTP-backed Solana RPC client.
 */
impl ReconciliationRpc for SolanaRpcClient {
    async fn get_signature_status(
        &self,
        signature: &str,
    ) -> Result<SignatureStatusLookup, SolanaRpcError> {
        SolanaRpcClient::get_signature_status(self, signature).await
    }

    async fn get_finalized_block_height(&self) -> Result<u64, SolanaRpcError> {
        SolanaRpcClient::get_finalized_block_height(self).await
    }

    async fn get_finalized_transaction(
        &self,
        signature: &str,
    ) -> Result<Option<FinalizedTransaction>, SolanaRpcError> {
        SolanaRpcClient::get_finalized_transaction(self, signature).await
    }

    async fn get_finalized_accounts(
        &self,
        addresses: &[String],
        min_context_slot: u64,
    ) -> Result<FinalizedAccountsLookup, SolanaRpcError> {
        SolanaRpcClient::get_finalized_accounts(self, addresses, min_context_slot).await
    }
}

/*
 * Successful processing outcomes.
 *
 * These are useful for worker logs and metrics. They describe what the
 * worker accomplished, without forcing the worker loop to inspect raw
 * database or RPC values.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitializeOrganizationProcessingResult {
    Deferred {
        submission_status: TransactionSubmissionStatus,
    },

    Expired,

    FinalizedFailure {
        transaction_slot: u64,
    },

    FinalizedSuccess {
        transaction_slot: u64,
        account_observed_slot: u64,
        block_time: Option<i64>,
    },
}

#[derive(Debug, Error)]
pub enum InitializeOrganizationProcessingError {
    #[error(transparent)]
    Load(#[from] ReconciliationLoadError),

    #[error(transparent)]
    PdaDerivation(#[from] PdaDerivationError),

    #[error(transparent)]
    Rpc(#[from] SolanaRpcError),

    #[error(transparent)]
    SignatureClassification(#[from] SignatureClassificationError),

    #[error(
        "signature status reported finalized transaction {signature}, \
         but getTransaction did not return it"
    )]
    FinalizedTransactionNotFound { signature: String },

    #[error(
        "signature status reported transaction slot {status_slot}, \
         but getTransaction returned slot {transaction_slot}"
    )]
    TransactionSlotMismatch {
        status_slot: u64,
        transaction_slot: u64,
    },

    #[error(transparent)]
    TransactionValidation(#[from] TransactionValidationError),

    #[error(transparent)]
    AccountValidation(#[from] AccountValidationError),

    #[error(transparent)]
    Settlement(#[from] ReconciliationSettlementError),
}

impl InitializeOrganizationProcessingError {
    /*
     * LeaseLost is different from an ordinary processing failure.
     *
     * It means this worker no longer owns the job. Therefore, the worker
     * loop must not call record_job_failure, because doing so would be
     * another stale write attempt.
     */
    pub fn is_lease_lost(&self) -> bool {
        matches!(
            self,
            Self::Settlement(ReconciliationSettlementError::JobQueue(
                JobQueueError::LeaseLost { .. }
            ))
        )
    }
}

/*
 * Process one already-claimed initialize_organization reconciliation job.
 *
 * Important transaction boundary:
 *
 * - Context loading uses a normal pooled connection.
 * - Solana RPC calls happen without a PostgreSQL transaction.
 * - Every settlement function opens its own short transaction.
 *
 * The worker loop is responsible for:
 *
 * - claiming the job;
 * - calling this function;
 * - recording operational failure when this function returns an error.
 */
pub async fn process_claimed_initialize_organization_job<R>(
    pool: &PgPool,
    rpc: &R,
    worker_cluster: &str,
    worker_program_id: &str,
    claimed_job: &ClaimedJob,
    defer_seconds: i64,
) -> Result<InitializeOrganizationProcessingResult, InitializeOrganizationProcessingError>
where
    R: ReconciliationRpc + ?Sized,
{
    /*
     * Phase 1: load the application facts that the chain must prove.
     *
     * This connection is released back to the pool before the first RPC
     * request. No database lock is held while waiting for Solana.
     */
    let context: InitializeOrganizationReconciliationContext = {
        let mut connection = pool
            .acquire()
            .await
            .map_err(ReconciliationLoadError::Database)?;

        load_initialize_organization_context(
            &mut connection,
            claimed_job,
            worker_cluster,
            worker_program_id,
        )
        .await?
    };

    /*
     * Derive the addresses independently from the same immutable inputs
     * used by the Anchor program.
     *
     * We do not trust addresses supplied by a client or stored without
     * verification.
     */
    let pdas = derive_initialize_organization_pdas(
        worker_program_id,
        &context.signer_wallet,
        context.organization_id,
        context.owner_member_id,
    )?;

    /*
     * Phase 2: observe the transaction signature.
     */
    let signature_lookup = rpc
        .get_signature_status(&context.transaction_signature)
        .await?;

    /*
     * Block height is only needed when the signature is missing.
     *
     * If the signature exists, its confirmation status already tells us
     * what to do.
     */
    let current_finalized_block_height = if signature_lookup.found_status.is_none() {
        Some(rpc.get_finalized_block_height().await?)
    } else {
        None
    };

    let action = classify_signature_lookup(
        &signature_lookup,
        context.last_valid_block_height,
        current_finalized_block_height,
    )?;

    match action {
        SignatureReconciliationAction::Defer { submission_status } => {
            /*
             * Confirmed observations persist their transaction slot.
             *
             * Processed and unknown observations do not populate the
             * confirmed_slot column.
             */
            let confirmed_transaction_slot =
                if submission_status == TransactionSubmissionStatus::Confirmed {
                    signature_lookup
                        .found_status
                        .as_ref()
                        .map(|status| status.transaction_slot)
                } else {
                    None
                };

            settle_deferred_signature_observation(
                pool,
                claimed_job,
                context.submission_id,
                submission_status,
                confirmed_transaction_slot,
                defer_seconds,
            )
            .await?;

            return Ok(InitializeOrganizationProcessingResult::Deferred { submission_status });
        }

        SignatureReconciliationAction::Expired => {
            let outcome = TerminalSubmissionOutcome::Expired;

            settle_terminal_submission_failure(pool, claimed_job, &context, &outcome).await?;

            return Ok(InitializeOrganizationProcessingResult::Expired);
        }

        SignatureReconciliationAction::FinalizedFailure {
            transaction_slot,
            error,
        } => {
            let outcome = TerminalSubmissionOutcome::FinalizedFailure {
                transaction_slot,
                error,
            };

            settle_terminal_submission_failure(pool, claimed_job, &context, &outcome).await?;

            return Ok(InitializeOrganizationProcessingResult::FinalizedFailure {
                transaction_slot,
            });
        }

        SignatureReconciliationAction::InspectFinalizedTransaction { transaction_slot } => {
            /*
             * Continue below. Signature status alone is not sufficient
             * evidence to activate the organization.
             */
            process_finalized_initialize_organization(
                pool,
                rpc,
                claimed_job,
                &context,
                &pdas,
                transaction_slot,
            )
            .await
        }
    }
}

async fn process_finalized_initialize_organization<R>(
    pool: &PgPool,
    rpc: &R,
    claimed_job: &ClaimedJob,
    context: &InitializeOrganizationReconciliationContext,
    pdas: &crate::pdas::InitializeOrganizationPdas,
    status_transaction_slot: u64,
) -> Result<InitializeOrganizationProcessingResult, InitializeOrganizationProcessingError>
where
    R: ReconciliationRpc + ?Sized,
{
    /*
     * Phase 3: fetch and validate the complete finalized transaction.
     */
    let transaction = rpc
        .get_finalized_transaction(&context.transaction_signature)
        .await?
        .ok_or_else(
            || InitializeOrganizationProcessingError::FinalizedTransactionNotFound {
                signature: context.transaction_signature.clone(),
            },
        )?;

    let transaction_expectation = InitializeOrganizationExpectation {
        transaction_signature: context.transaction_signature.clone(),

        program_id: context.program_id.clone(),

        organization_id: context.organization_id,
        owner_member_id: context.owner_member_id,

        creator_wallet: context.signer_wallet.clone(),

        organization_pda: pdas.organization_pda.clone(),
        owner_member_pda: pdas.owner_member_pda.clone(),
        owner_member_wallet_pda: pdas.owner_member_wallet_pda.clone(),
    };

    let validated_transaction =
        validate_initialize_organization_transaction(&transaction, &transaction_expectation)?;

    /*
     * getSignatureStatuses and getTransaction are separate RPC calls.
     *
     * Both must describe the same transaction slot. A mismatch indicates
     * an inconsistent RPC response and must not be persisted.
     */
    if validated_transaction.transaction_slot != status_transaction_slot {
        return Err(
            InitializeOrganizationProcessingError::TransactionSlotMismatch {
                status_slot: status_transaction_slot,
                transaction_slot: validated_transaction.transaction_slot,
            },
        );
    }

    /*
     * Phase 4: fetch the three accounts from one finalized RPC context.
     *
     * Their order is important because getMultipleAccounts returns values
     * positionally.
     */
    let addresses = vec![
        pdas.organization_pda.clone(),
        pdas.owner_member_pda.clone(),
        pdas.owner_member_wallet_pda.clone(),
    ];

    let accounts_lookup = rpc
        .get_finalized_accounts(&addresses, validated_transaction.transaction_slot)
        .await?;

    let accounts_expectation = InitializeOrganizationAccountsExpectation {
        program_id: context.program_id.clone(),

        organization_pda: pdas.organization_pda.clone(),
        owner_member_pda: pdas.owner_member_pda.clone(),
        owner_member_wallet_pda: pdas.owner_member_wallet_pda.clone(),

        organization_id: context.organization_id,
        owner_member_id: context.owner_member_id,
        creator_wallet: context.signer_wallet.clone(),

        organization_bump: pdas.organization_bump,
        owner_member_bump: pdas.owner_member_bump,
        owner_member_wallet_bump: pdas.owner_member_wallet_bump,
    };

    let validated_accounts =
        validate_initialize_organization_accounts(&accounts_lookup, &accounts_expectation)?;

    /*
     * Phase 5: persist the authoritative finalized result.
     *
     * This function atomically:
     *
     * - finalizes the submission;
     * - upserts all three chain projections;
     * - activates the organization;
     * - activates the owner member;
     * - marks the job successful.
     */
    settle_finalized_initialize_organization(
        pool,
        claimed_job,
        context,
        validated_transaction.transaction_slot,
        &validated_accounts,
    )
    .await?;

    Ok(InitializeOrganizationProcessingResult::FinalizedSuccess {
        transaction_slot: validated_transaction.transaction_slot,

        account_observed_slot: validated_accounts.observed_slot,

        block_time: validated_transaction.block_time,
    })
}
