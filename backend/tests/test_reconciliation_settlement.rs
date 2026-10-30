use std::{env, error::Error, sync::Mutex, time::Duration};

use bigdecimal::BigDecimal;
use payout_backend::{
    TransactionSubmissionStatus,
    account_validation::{
        ValidatedInitializeOrganizationAccounts, ValidatedMemberAccount,
        ValidatedMemberWalletAccount, ValidatedOrganizationAccount,
    },
    jobs::{ClaimedJob, JobQueueError, claim_next_job},
    pdas::derive_initialize_organization_pdas,
    reconciliation::{
        InitializeOrganizationReconciliationContext, ReconciliationSettlementError,
        TerminalSubmissionOutcome, settle_deferred_signature_observation,
        settle_finalized_initialize_organization, settle_terminal_submission_failure,
    },
    reconciliation_worker::{
        InitializeOrganizationProcessingResult, ReconciliationRpc,
        process_claimed_initialize_organization_job,
    },
    solana_rpc::{
        ConfirmationStatus, FinalizedAccountSnapshot, FinalizedAccountsLookup,
        FinalizedTransaction, FoundSignatureStatus, SignatureStatusLookup, SolanaRpcError,
        TransactionInstruction,
    },
};
use serde_json::json;
use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const SYSTEM_PROGRAM_ID: &str = "11111111111111111111111111111111";

const PROGRAM_ID: &str = "5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv";

const SIGNER_WALLET: &str = "4k5kaSZ796EWM8hNfXuhcdVbZ2MNWonTbRmsshWDBX7b";

const RECONCILIATION_JOB_TYPE: &str = "reconcile_transaction_submission";

const INITIALIZE_ORGANIZATION_DISCRIMINATOR: [u8; 8] = [21, 20, 253, 138, 250, 160, 119, 87];

const ORGANIZATION_ACCOUNT_DISCRIMINATOR: [u8; 8] = [145, 38, 152, 251, 91, 57, 118, 160];

const MEMBER_ACCOUNT_DISCRIMINATOR: [u8; 8] = [54, 19, 162, 21, 29, 166, 17, 198];

const MEMBER_WALLET_ACCOUNT_DISCRIMINATOR: [u8; 8] = [225, 26, 52, 143, 86, 46, 6, 54];

fn pubkey_bytes(address: &str) -> [u8; 32] {
    let decoded = bs58::decode(address)
        .into_vec()
        .expect("test public key should be valid Base58");

    decoded
        .try_into()
        .expect("test public key should contain 32 bytes")
}

fn initialize_organization_instruction_data(organization_id: u64, owner_member_id: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(24);

    data.extend_from_slice(&INITIALIZE_ORGANIZATION_DISCRIMINATOR);

    data.extend_from_slice(&organization_id.to_le_bytes());

    data.extend_from_slice(&owner_member_id.to_le_bytes());

    data
}

fn organization_account_data(
    organization_id: u64,
    creator_wallet: &str,
    owner_member_pda: &str,
    bump: u8,
) -> Vec<u8> {
    let mut data = Vec::new();

    data.extend_from_slice(&ORGANIZATION_ACCOUNT_DISCRIMINATOR);

    data.extend_from_slice(&organization_id.to_le_bytes());

    data.extend_from_slice(&pubkey_bytes(creator_wallet));

    data.extend_from_slice(&pubkey_bytes(owner_member_pda));

    // paused = false
    data.push(0);

    data.push(bump);

    data
}

fn member_account_data(
    organization_pda: &str,
    member_id: u64,
    authorized_wallet: &str,
    bump: u8,
) -> Vec<u8> {
    let mut data = Vec::new();

    data.extend_from_slice(&MEMBER_ACCOUNT_DISCRIMINATOR);

    data.extend_from_slice(&pubkey_bytes(organization_pda));

    data.extend_from_slice(&member_id.to_le_bytes());

    data.extend_from_slice(&pubkey_bytes(authorized_wallet));

    // authorization_revision = 1
    data.extend_from_slice(&1_u64.to_le_bytes());

    // Initial owner roles: ROLE_OWNER | ROLE_ADMIN = 3
    data.extend_from_slice(&3_u16.to_le_bytes());

    // active = true
    data.push(1);

    data.push(bump);

    data
}

fn member_wallet_account_data(
    organization_pda: &str,
    member_pda: &str,
    authorized_wallet: &str,
    bump: u8,
) -> Vec<u8> {
    let mut data = Vec::new();

    data.extend_from_slice(&MEMBER_WALLET_ACCOUNT_DISCRIMINATOR);

    data.extend_from_slice(&pubkey_bytes(organization_pda));

    data.extend_from_slice(&pubkey_bytes(member_pda));

    data.extend_from_slice(&pubkey_bytes(authorized_wallet));

    data.push(bump);

    data
}

struct FinalizedSuccessRpc {
    expected_signature: String,
    expected_addresses: Vec<String>,
    expected_min_context_slot: u64,

    signature_lookup: SignatureStatusLookup,
    transaction: FinalizedTransaction,
    accounts: FinalizedAccountsLookup,

    calls: Mutex<FakeRpcCalls>,
}

impl FinalizedSuccessRpc {
    fn calls(&self) -> FakeRpcCalls {
        *self
            .calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
    }

    fn invalid_request(message: impl Into<String>) -> SolanaRpcError {
        SolanaRpcError::InvalidResponse(message.into())
    }
}

impl ReconciliationRpc for FinalizedSuccessRpc {
    async fn get_signature_status(
        &self,
        signature: &str,
    ) -> Result<SignatureStatusLookup, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .signature_status += 1;

        if signature != self.expected_signature {
            return Err(Self::invalid_request(format!(
                "expected signature {}, received {signature}",
                self.expected_signature,
            )));
        }

        Ok(self.signature_lookup.clone())
    }

    async fn get_finalized_block_height(&self) -> Result<u64, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .finalized_block_height += 1;

        Err(Self::invalid_request(
            "finalized success must not request block height",
        ))
    }

    async fn get_finalized_transaction(
        &self,
        signature: &str,
    ) -> Result<Option<FinalizedTransaction>, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .finalized_transaction += 1;

        if signature != self.expected_signature {
            return Err(Self::invalid_request(format!(
                "expected transaction signature {}, received {signature}",
                self.expected_signature,
            )));
        }

        Ok(Some(self.transaction.clone()))
    }

    async fn get_finalized_accounts(
        &self,
        addresses: &[String],
        min_context_slot: u64,
    ) -> Result<FinalizedAccountsLookup, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .finalized_accounts += 1;

        if addresses != self.expected_addresses.as_slice() {
            return Err(Self::invalid_request(format!(
                "expected account request {:?}, received {addresses:?}",
                self.expected_addresses,
            )));
        }

        if min_context_slot != self.expected_min_context_slot {
            return Err(Self::invalid_request(format!(
                "expected minimum context slot {}, received {min_context_slot}",
                self.expected_min_context_slot,
            )));
        }

        Ok(self.accounts.clone())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct FakeRpcCalls {
    signature_status: usize,
    finalized_block_height: usize,
    finalized_transaction: usize,
    finalized_accounts: usize,
}

struct FakeRpc {
    signature_lookup: SignatureStatusLookup,
    finalized_block_height: Option<u64>,
    calls: Mutex<FakeRpcCalls>,
}

impl FakeRpc {
    fn confirmed(transaction_slot: u64) -> Self {
        Self {
            signature_lookup: SignatureStatusLookup {
                context_slot: transaction_slot + 100,
                found_status: Some(FoundSignatureStatus {
                    transaction_slot,
                    confirmation_status: ConfirmationStatus::Confirmed,
                    error: None,
                }),
            },

            finalized_block_height: None,
            calls: Mutex::new(FakeRpcCalls::default()),
        }
    }

    fn missing(finalized_block_height: u64) -> Self {
        Self {
            signature_lookup: SignatureStatusLookup {
                context_slot: finalized_block_height,
                found_status: None,
            },

            finalized_block_height: Some(finalized_block_height),

            calls: Mutex::new(FakeRpcCalls::default()),
        }
    }

    fn calls(&self) -> FakeRpcCalls {
        *self
            .calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
    }

    fn unexpected_call(method: &str) -> SolanaRpcError {
        SolanaRpcError::InvalidResponse(format!("test fake received unexpected {method} call"))
    }
}

impl ReconciliationRpc for FakeRpc {
    async fn get_signature_status(
        &self,
        _signature: &str,
    ) -> Result<SignatureStatusLookup, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .signature_status += 1;

        Ok(self.signature_lookup.clone())
    }

    async fn get_finalized_block_height(&self) -> Result<u64, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .finalized_block_height += 1;

        self.finalized_block_height
            .ok_or_else(|| Self::unexpected_call("get_finalized_block_height"))
    }

    async fn get_finalized_transaction(
        &self,
        _signature: &str,
    ) -> Result<Option<FinalizedTransaction>, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .finalized_transaction += 1;

        Err(Self::unexpected_call("get_finalized_transaction"))
    }

    async fn get_finalized_accounts(
        &self,
        _addresses: &[String],
        _min_context_slot: u64,
    ) -> Result<FinalizedAccountsLookup, SolanaRpcError> {
        self.calls
            .lock()
            .expect("fake RPC call counter lock should not be poisoned")
            .finalized_accounts += 1;

        Err(Self::unexpected_call("get_finalized_accounts"))
    }
}

struct TestFixture {
    organization_id: BigDecimal,
    owner_member_id: BigDecimal,

    submission_id: i64,
    transaction_signature: String,

    job_id: i64,
    job_type: String,
}

async fn connect_test_pool() -> Result<PgPool, Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    Ok(pool)
}

async fn insert_test_fixture(pool: &PgPool) -> Result<TestFixture, Box<dyn Error>> {
    /*
     * The settlement function opens its own transaction.
     *
     * Therefore, fixtures must be committed first. A different database
     * transaction cannot see uncommitted fixture rows.
     */
    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();

    let organization_name = format!("Reconciliation Settlement Test {test_id}");

    let organization_id = sqlx::query_scalar!(
        r#"
        INSERT INTO organizations (
            name,
            status
        )
        VALUES (
            $1,
            'provisioning'
        )
        RETURNING organization_id
        "#,
        &organization_name,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let owner_display_name = format!("Settlement Test Owner {test_id}");

    let owner_external_reference = format!("settlement-test-owner-{test_id}");

    let owner_member_id = sqlx::query_scalar!(
        r#"
    INSERT INTO organization_members (
        organization_id,
        display_name,
        external_reference,
        requested_authorized_wallet,
        requested_roles,
        lifecycle_status
    )
    VALUES (
        $1,
        $2,
        $3,
        $4,
        1,
        'provisioning'
    )
    RETURNING member_id
    "#,
        &organization_id,
        &owner_display_name,
        &owner_external_reference,
        SIGNER_WALLET,
    )
    .fetch_one(&mut *transaction)
    .await?;

    /*
     * The schema currently checks signature length, not its cryptographic
     * contents. This unique 64-character value is sufficient for a database
     * settlement test; Solana transaction validation is tested separately.
     */
    let transaction_signature = Uuid::new_v4().simple().to_string().repeat(2);

    let submission_id = sqlx::query_scalar!(
        r#"
        INSERT INTO transaction_submissions (
            idempotency_key,
            organization_id,
            cluster,
            program_id,
            instruction_name,
            signer_wallet,
            entity_key,
            request_payload,
            transaction_signature,
            last_valid_block_height,
            status,
            submitted_at
        )
        VALUES (
            $1,
            $2,
            'devnet',
            $3,
            'initialize_organization',
            $4,
            $5,
            $6,
            $7,
            1000000000,
            'submitted',
            now()
        )
        RETURNING submission_id
        "#,
        Uuid::new_v4(),
        &organization_id,
        PROGRAM_ID,
        SIGNER_WALLET,
        json!({
            "organization_id": organization_id.to_string(),
        }),
        json!({
            "test": true,
        }),
        &transaction_signature,
    )
    .fetch_one(&mut *transaction)
    .await?;

    /*
     * Use a unique job type.
     *
     * claim_next_job filters by job type, so this prevents the test from
     * claiming a real reconciliation job already stored in the development
     * database.
     */
    let job_type = format!("test_reconciliation_settlement_{test_id}");

    let job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            organization_id,
            submission_id,
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5,
            10
        )
        RETURNING job_id
        "#,
        &organization_id,
        submission_id,
        &job_type,
        json!({
            "test": true,
            "purpose": "reconciliation settlement",
        }),
        format!("test-reconciliation-settlement:{test_id}"),
    )
    .fetch_one(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(TestFixture {
        organization_id,
        owner_member_id,

        submission_id,
        transaction_signature,

        job_id,
        job_type,
    })
}

async fn claim_fixture_job(
    pool: &PgPool,
    fixture: &TestFixture,
    worker_id: &str,
) -> Result<ClaimedJob, Box<dyn Error>> {
    /*
     * Claim and commit before calling settlement.
     *
     * In production, the worker also commits its short claim transaction
     * before contacting Solana RPC.
     */
    let mut transaction = pool.begin().await?;

    let claimed_job = claim_next_job(&mut transaction, worker_id, &fixture.job_type, 30)
        .await?
        .expect("the fixture job should be claimable");

    transaction.commit().await?;

    assert_eq!(claimed_job.job_id, fixture.job_id);
    assert_eq!(claimed_job.submission_id, Some(fixture.submission_id));

    Ok(claimed_job)
}

fn reconciliation_context(fixture: &TestFixture) -> InitializeOrganizationReconciliationContext {
    let organization_id = fixture
        .organization_id
        .to_string()
        .parse::<u64>()
        .expect("test organization ID should fit in u64");

    let owner_member_id = fixture
        .owner_member_id
        .to_string()
        .parse::<u64>()
        .expect("test owner member ID should fit in u64");

    InitializeOrganizationReconciliationContext {
        submission_id: fixture.submission_id,

        organization_id,
        owner_member_id,

        cluster: "devnet".to_owned(),
        program_id: PROGRAM_ID.to_owned(),
        signer_wallet: SIGNER_WALLET.to_owned(),

        transaction_signature: fixture.transaction_signature.clone(),
        last_valid_block_height: 1_000_000_000,
        submission_status: TransactionSubmissionStatus::Submitted,

        organization_status: "provisioning".to_owned(),
        owner_member_status: "provisioning".to_owned(),
    }
}

fn unique_pubkey() -> String {
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();

    let mut bytes = [0_u8; 32];

    bytes[..16].copy_from_slice(first.as_bytes());
    bytes[16..].copy_from_slice(second.as_bytes());

    bs58::encode(bytes).into_string()
}

fn validated_accounts(
    fixture: &TestFixture,
    observed_slot: u64,
) -> ValidatedInitializeOrganizationAccounts {
    let organization_id = fixture
        .organization_id
        .to_string()
        .parse::<u64>()
        .expect("test organization ID should fit in u64");

    let owner_member_id = fixture
        .owner_member_id
        .to_string()
        .parse::<u64>()
        .expect("test member ID should fit in u64");

    let organization_pda = unique_pubkey();
    let owner_member_pda = unique_pubkey();
    let owner_member_wallet_pda = unique_pubkey();

    ValidatedInitializeOrganizationAccounts {
        observed_slot,

        organization: ValidatedOrganizationAccount {
            address: organization_pda.clone(),
            organization_id,
            creator_wallet: SIGNER_WALLET.to_owned(),
            owner_member_pda: owner_member_pda.clone(),
            paused: false,
            bump: 250,
        },

        owner_member: ValidatedMemberAccount {
            address: owner_member_pda.clone(),
            organization_pda: organization_pda.clone(),
            member_id: owner_member_id,

            authorized_wallet: SIGNER_WALLET.to_owned(),
            authorization_revision: 1,

            roles: 3,
            active: true,
            bump: 249,
        },

        owner_member_wallet: ValidatedMemberWalletAccount {
            address: owner_member_wallet_pda,
            organization_pda,
            member_pda: owner_member_pda,
            authorized_wallet: SIGNER_WALLET.to_owned(),
            bump: 248,
        },
    }
}

async fn cleanup_fixture(pool: &PgPool, fixture: &TestFixture) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;

    /*
     * Delete children before parents because all relevant foreign keys use
     * ON DELETE RESTRICT.
     */
    sqlx::query!(
        r#"
        DELETE FROM transaction_submission_status_history
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
    DELETE FROM member_wallet_chain_accounts
    WHERE organization_id = $1
    "#,
        &fixture.organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
    DELETE FROM member_chain_accounts
    WHERE organization_id = $1
    "#,
        &fixture.organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
    DELETE FROM organization_chain_accounts
    WHERE organization_id = $1
    "#,
        &fixture.organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM organization_members
        WHERE organization_id = $1
        "#,
        &fixture.organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
        DELETE FROM organizations
        WHERE organization_id = $1
        "#,
        &fixture.organization_id,
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}

#[tokio::test]
async fn confirmed_observation_updates_submission_and_defers_job_atomically()
-> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("confirmed-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    settle_deferred_signature_observation(
        &pool,
        &claimed_job,
        fixture.submission_id,
        TransactionSubmissionStatus::Confirmed,
        Some(875_000_000),
        60,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            last_checked_at,
            confirmed_at,
            error_code,
            error_message
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            run_at > now() AS "deferred!",
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let confirmed_transition_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
          AND previous_status = 'submitted'
          AND new_status = 'confirmed'
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "confirmed");
    assert_eq!(stored_submission.confirmed_slot, Some(875_000_000));
    assert_eq!(
        stored_submission.observed_commitment.as_deref(),
        Some("confirmed")
    );
    assert!(stored_submission.last_checked_at.is_some());
    assert!(stored_submission.confirmed_at.is_some());
    assert!(stored_submission.error_code.is_none());
    assert!(stored_submission.error_message.is_none());

    assert_eq!(stored_job.status, "queued");
    assert_eq!(stored_job.attempts, 1);
    assert!(stored_job.deferred);
    assert!(stored_job.locked_at.is_none());
    assert!(stored_job.locked_by.is_none());
    assert!(stored_job.lease_expires_at.is_none());
    assert!(stored_job.completed_at.is_none());

    assert_eq!(confirmed_transition_count, 1);

    Ok(())
}

#[tokio::test]
async fn stale_rpc_observation_does_not_regress_confirmed_submission() -> Result<(), Box<dyn Error>>
{
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    /*
     * Establish a previously confirmed state before the worker claims the
     * reconciliation job.
     */
    sqlx::query!(
        r#"
        UPDATE transaction_submissions
        SET
            status = 'confirmed',
            confirmed_slot = 700,
            observed_commitment = 'confirmed',
            confirmed_at = now(),
            last_checked_at = now()
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .execute(&pool)
    .await?;

    let history_count_before = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let worker_id = format!("stale-rpc-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    /*
     * Simulate a stale RPC node reporting only processed commitment.
     */
    settle_deferred_signature_observation(
        &pool,
        &claimed_job,
        fixture.submission_id,
        TransactionSubmissionStatus::Submitted,
        None,
        60,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            confirmed_at
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let history_count_after = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job_status = sqlx::query_scalar!(
        r#"
        SELECT status
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "confirmed");
    assert_eq!(stored_submission.confirmed_slot, Some(700));
    assert_eq!(
        stored_submission.observed_commitment.as_deref(),
        Some("confirmed")
    );
    assert!(stored_submission.confirmed_at.is_some());

    /*
     * The UPDATE statement ran, but status remained confirmed.
     *
     * Therefore, the trigger's IS DISTINCT FROM check must not append a fake
     * confirmed -> confirmed history record.
     */
    assert_eq!(history_count_after, history_count_before);
    assert_eq!(stored_job_status, "queued");

    Ok(())
}

#[tokio::test]
async fn stale_worker_failure_rolls_back_submission_update() -> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("stale-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    /*
     * Simulate lease ownership moving away from the worker.
     *
     * The ClaimedJob in memory still identifies the original worker, but the
     * database row now belongs to another worker identity.
     */
    let replacement_worker_id = format!("replacement-worker:{}", Uuid::new_v4());

    sqlx::query!(
        r#"
        UPDATE jobs
        SET locked_by = $2
        WHERE job_id = $1
          AND status = 'running'
        "#,
        fixture.job_id,
        &replacement_worker_id,
    )
    .execute(&pool)
    .await?;

    let settlement_result = settle_deferred_signature_observation(
        &pool,
        &claimed_job,
        fixture.submission_id,
        TransactionSubmissionStatus::Confirmed,
        Some(875_000_100),
        60,
    )
    .await;

    assert!(matches!(
        settlement_result,
        Err(ReconciliationSettlementError::JobQueue(
            JobQueueError::LeaseLost { .. }
        ))
    ));

    /*
     * settle_deferred_signature_observation updated the submission first.
     *
     * However, defer_job then detected the lost lease. Because both operations
     * were inside one transaction, returning the error rolled back the earlier
     * submission update.
     */
    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            confirmed_at
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let history_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            locked_by
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "submitted");
    assert!(stored_submission.confirmed_slot.is_none());
    assert!(stored_submission.observed_commitment.is_none());
    assert!(stored_submission.confirmed_at.is_none());

    /*
     * Only the initial NULL -> submitted history row should exist.
     * The rolled-back submitted -> confirmed row must not remain.
     */
    assert_eq!(history_count, 1);

    assert_eq!(stored_job.status, "running");
    assert_eq!(
        stored_job.locked_by.as_deref(),
        Some(replacement_worker_id.as_str())
    );

    Ok(())
}

#[tokio::test]
async fn expired_submission_restores_drafts_and_completes_job() -> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("expired-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    let context = reconciliation_context(&fixture);

    settle_terminal_submission_failure(
        &pool,
        &claimed_job,
        &context,
        &TerminalSubmissionOutcome::Expired,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            last_checked_at,
            confirmed_at,
            finalized_at,
            failed_at,
            error_code,
            error_message
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let organization_status = sqlx::query_scalar!(
        r#"
        SELECT status
        FROM organizations
        WHERE organization_id = $1
        "#,
        &fixture.organization_id,
    )
    .fetch_one(&pool)
    .await?;

    let owner_member_status = sqlx::query_scalar!(
        r#"
        SELECT lifecycle_status
        FROM organization_members
        WHERE organization_id = $1
          AND member_id = $2
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let expired_transition_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
          AND previous_status = 'submitted'
          AND new_status = 'expired'
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "expired");
    assert!(stored_submission.confirmed_slot.is_none());
    assert!(stored_submission.observed_commitment.is_none());
    assert!(stored_submission.last_checked_at.is_some());
    assert!(stored_submission.confirmed_at.is_none());
    assert!(stored_submission.finalized_at.is_none());

    /*
     * The schema has failed_at but no separate expired_at column, so failed_at
     * records when this terminal unsuccessful outcome was persisted.
     */
    assert!(stored_submission.failed_at.is_some());

    assert_eq!(
        stored_submission.error_code.as_deref(),
        Some("transaction_expired")
    );

    assert!(
        stored_submission
            .error_message
            .as_deref()
            .is_some_and(|message| { message.contains("1000000000") })
    );

    assert_eq!(organization_status, "draft");
    assert_eq!(owner_member_status, "draft");

    assert_eq!(stored_job.status, "succeeded");
    assert_eq!(stored_job.attempts, 1);
    assert!(stored_job.locked_at.is_none());
    assert!(stored_job.locked_by.is_none());
    assert!(stored_job.lease_expires_at.is_none());
    assert!(stored_job.completed_at.is_some());

    assert_eq!(expired_transition_count, 1);

    Ok(())
}

#[tokio::test]
async fn finalized_chain_failure_is_persisted_and_job_succeeds() -> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("finalized-failure-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    let context = reconciliation_context(&fixture);

    let chain_error = json!({
        "InstructionError": [
            0,
            {
                "Custom": 6001
            }
        ]
    });

    let expected_error_message = chain_error.to_string();

    settle_terminal_submission_failure(
        &pool,
        &claimed_job,
        &context,
        &TerminalSubmissionOutcome::FinalizedFailure {
            transaction_slot: 875_000_500,
            error: chain_error,
        },
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            last_checked_at,
            confirmed_at,
            finalized_at,
            failed_at,
            error_code,
            error_message
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let lifecycle = sqlx::query!(
        r#"
        SELECT
            organization.status AS organization_status,
            owner_member.lifecycle_status AS owner_member_status

        FROM organizations AS organization

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             organization.organization_id
         AND owner_member.member_id = $2

        WHERE organization.organization_id = $1
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let failed_transition_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
          AND previous_status = 'submitted'
          AND new_status = 'failed'
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "failed");

    assert_eq!(stored_submission.confirmed_slot, Some(875_000_500));

    assert_eq!(
        stored_submission.observed_commitment.as_deref(),
        Some("finalized")
    );

    assert!(stored_submission.last_checked_at.is_some());
    assert!(stored_submission.confirmed_at.is_some());
    assert!(stored_submission.finalized_at.is_some());
    assert!(stored_submission.failed_at.is_some());

    assert_eq!(
        stored_submission.error_code.as_deref(),
        Some("solana_transaction_failed")
    );

    assert_eq!(
        stored_submission.error_message.as_deref(),
        Some(expected_error_message.as_str())
    );

    assert_eq!(lifecycle.organization_status, "draft");
    assert_eq!(lifecycle.owner_member_status, "draft");

    /*
     * Solana execution failed, but reconciliation itself completed
     * successfully and persisted the authoritative outcome.
     */
    assert_eq!(stored_job.status, "succeeded");
    assert!(stored_job.locked_at.is_none());
    assert!(stored_job.locked_by.is_none());
    assert!(stored_job.lease_expires_at.is_none());
    assert!(stored_job.completed_at.is_some());

    assert_eq!(failed_transition_count, 1);

    Ok(())
}

#[tokio::test]
async fn stale_worker_rolls_back_terminal_settlement() -> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("terminal-stale-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    let replacement_worker_id = format!("replacement-worker:{}", Uuid::new_v4());

    /*
     * Change logical ownership after the original ClaimedJob was produced.
     * The in-memory claim is now stale.
     */
    sqlx::query!(
        r#"
        UPDATE jobs
        SET locked_by = $2
        WHERE job_id = $1
          AND status = 'running'
        "#,
        fixture.job_id,
        &replacement_worker_id,
    )
    .execute(&pool)
    .await?;

    let context = reconciliation_context(&fixture);

    let result = settle_terminal_submission_failure(
        &pool,
        &claimed_job,
        &context,
        &TerminalSubmissionOutcome::Expired,
    )
    .await;

    assert!(matches!(
        result,
        Err(ReconciliationSettlementError::JobQueue(
            JobQueueError::LeaseLost { .. }
        ))
    ));

    /*
     * The settlement function updates these rows before it calls
     * mark_job_succeeded().
     *
     * The fencing failure must therefore roll back:
     *
     * - submission expired update;
     * - organization draft update;
     * - owner member draft update;
     * - generated status-history row.
     */
    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            failed_at,
            error_code,
            error_message
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let lifecycle = sqlx::query!(
        r#"
        SELECT
            organization.status AS organization_status,
            owner_member.lifecycle_status AS owner_member_status

        FROM organizations AS organization

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             organization.organization_id
         AND owner_member.member_id = $2

        WHERE organization.organization_id = $1
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let history_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            locked_by,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "submitted");
    assert!(stored_submission.failed_at.is_none());
    assert!(stored_submission.error_code.is_none());
    assert!(stored_submission.error_message.is_none());

    assert_eq!(lifecycle.organization_status, "provisioning");

    assert_eq!(lifecycle.owner_member_status, "provisioning");

    /*
     * Only the trigger-generated NULL -> submitted history row remains.
     * The submitted -> expired entry was part of the rolled-back transaction.
     */
    assert_eq!(history_count, 1);

    assert_eq!(stored_job.status, "running");
    assert_eq!(
        stored_job.locked_by.as_deref(),
        Some(replacement_worker_id.as_str())
    );
    assert!(stored_job.completed_at.is_none());

    Ok(())
}

#[tokio::test]
async fn finalized_success_writes_projections_and_activates_application()
-> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("finalized-success-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    let context = reconciliation_context(&fixture);

    let transaction_slot = 900_000_000;
    let accounts = validated_accounts(&fixture, 900_000_100);

    settle_finalized_initialize_organization(
        &pool,
        &claimed_job,
        &context,
        transaction_slot,
        &accounts,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            confirmed_at,
            finalized_at,
            failed_at,
            error_code,
            error_message
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let organization_projection = sqlx::query!(
        r#"
        SELECT
            organization_pda,
            creator_wallet,
            owner_member_pda,
            paused,
            bump,
            observed_slot,
            observed_commitment
        FROM organization_chain_accounts
        WHERE organization_id = $1
          AND cluster = 'devnet'
          AND program_id = $2
        "#,
        &fixture.organization_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let member_projection = sqlx::query!(
        r#"
        SELECT
            organization_pda,
            member_pda,
            authorized_wallet,
            authorization_revision,
            roles,
            active,
            bump,
            observed_slot,
            observed_commitment
        FROM member_chain_accounts
        WHERE organization_id = $1
          AND member_id = $2
          AND cluster = 'devnet'
          AND program_id = $3
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let member_wallet_projection = sqlx::query!(
        r#"
        SELECT
            organization_pda,
            member_pda,
            member_wallet_pda,
            authorized_wallet,
            bump,
            observed_slot,
            observed_commitment
        FROM member_wallet_chain_accounts
        WHERE organization_id = $1
          AND member_id = $2
          AND cluster = 'devnet'
          AND program_id = $3
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let lifecycle = sqlx::query!(
        r#"
        SELECT
            organization.status AS organization_status,
            owner_member.lifecycle_status AS owner_member_status

        FROM organizations AS organization

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             organization.organization_id
         AND owner_member.member_id = $2

        WHERE organization.organization_id = $1
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let finalized_transition_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
          AND previous_status = 'submitted'
          AND new_status = 'finalized'
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(stored_submission.status, "finalized");

    assert_eq!(
        stored_submission.confirmed_slot,
        Some(transaction_slot as i64)
    );

    assert_eq!(
        stored_submission.observed_commitment.as_deref(),
        Some("finalized")
    );

    assert!(stored_submission.confirmed_at.is_some());
    assert!(stored_submission.finalized_at.is_some());
    assert!(stored_submission.failed_at.is_none());
    assert!(stored_submission.error_code.is_none());
    assert!(stored_submission.error_message.is_none());

    assert_eq!(
        organization_projection.organization_pda,
        accounts.organization.address
    );

    assert_eq!(
        organization_projection.creator_wallet,
        accounts.organization.creator_wallet
    );

    assert_eq!(
        organization_projection.owner_member_pda,
        accounts.organization.owner_member_pda
    );

    assert!(!organization_projection.paused);
    assert_eq!(organization_projection.bump, 250);
    assert_eq!(
        organization_projection.observed_slot,
        accounts.observed_slot as i64
    );
    assert_eq!(organization_projection.observed_commitment, "finalized");

    assert_eq!(
        member_projection.organization_pda,
        accounts.owner_member.organization_pda
    );

    assert_eq!(member_projection.member_pda, accounts.owner_member.address);

    assert_eq!(
        member_projection.authorized_wallet,
        accounts.owner_member.authorized_wallet
    );

    assert_eq!(
        member_projection.authorization_revision,
        BigDecimal::from(1_u64)
    );

    assert_eq!(member_projection.roles, 3);
    assert!(member_projection.active);
    assert_eq!(member_projection.bump, 249);
    assert_eq!(
        member_projection.observed_slot,
        accounts.observed_slot as i64
    );
    assert_eq!(member_projection.observed_commitment, "finalized");

    assert_eq!(
        member_wallet_projection.organization_pda,
        accounts.owner_member_wallet.organization_pda
    );

    assert_eq!(
        member_wallet_projection.member_pda,
        accounts.owner_member_wallet.member_pda
    );

    assert_eq!(
        member_wallet_projection.member_wallet_pda,
        accounts.owner_member_wallet.address
    );

    assert_eq!(
        member_wallet_projection.authorized_wallet,
        accounts.owner_member_wallet.authorized_wallet
    );

    assert_eq!(member_wallet_projection.bump, 248);
    assert_eq!(
        member_wallet_projection.observed_slot,
        accounts.observed_slot as i64
    );
    assert_eq!(member_wallet_projection.observed_commitment, "finalized");

    assert_eq!(lifecycle.organization_status, "active");
    assert_eq!(lifecycle.owner_member_status, "active");

    assert_eq!(stored_job.status, "succeeded");
    assert_eq!(stored_job.attempts, 1);
    assert!(stored_job.locked_at.is_none());
    assert!(stored_job.locked_by.is_none());
    assert!(stored_job.lease_expires_at.is_none());
    assert!(stored_job.completed_at.is_some());

    assert_eq!(finalized_transition_count, 1);

    Ok(())
}

async fn insert_projection_snapshot(
    pool: &PgPool,
    fixture: &TestFixture,
    accounts: &ValidatedInitializeOrganizationAccounts,
) -> Result<(), Box<dyn Error>> {
    let authorization_revision = BigDecimal::from(accounts.owner_member.authorization_revision);

    let observed_slot = i64::try_from(accounts.observed_slot)?;

    let mut transaction = pool.begin().await?;

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
            'devnet',
            $2,
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            'finalized'
        )
        "#,
        &fixture.organization_id,
        PROGRAM_ID,
        &accounts.organization.address,
        &accounts.organization.creator_wallet,
        &accounts.organization.owner_member_pda,
        accounts.organization.paused,
        i16::from(accounts.organization.bump),
        observed_slot,
    )
    .execute(&mut *transaction)
    .await?;

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
            'devnet',
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            'finalized'
        )
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
        &accounts.owner_member.organization_pda,
        &accounts.owner_member.address,
        &accounts.owner_member.authorized_wallet,
        &authorization_revision,
        i32::from(accounts.owner_member.roles),
        accounts.owner_member.active,
        i16::from(accounts.owner_member.bump),
        observed_slot,
    )
    .execute(&mut *transaction)
    .await?;

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
            'devnet',
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            'finalized'
        )
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
        &accounts.owner_member_wallet.organization_pda,
        &accounts.owner_member_wallet.member_pda,
        &accounts.owner_member_wallet.address,
        &accounts.owner_member_wallet.authorized_wallet,
        i16::from(accounts.owner_member_wallet.bump),
        observed_slot,
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}

#[tokio::test]
async fn older_account_snapshot_cannot_overwrite_newer_projections() -> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    /*
     * This represents a newer account synchronization that reached the
     * database before the delayed reconciliation worker.
     */
    let mut newer_accounts = validated_accounts(&fixture, 2_000);

    let newer_wallet = unique_pubkey();

    newer_accounts.organization.paused = true;

    newer_accounts.owner_member.authorized_wallet = newer_wallet.clone();
    newer_accounts.owner_member.authorization_revision = 2;
    newer_accounts.owner_member.roles = 7;
    newer_accounts.owner_member.active = false;

    newer_accounts.owner_member_wallet.authorized_wallet = newer_wallet.clone();

    insert_projection_snapshot(&pool, &fixture, &newer_accounts).await?;

    /*
     * Use the same PDA identities but older mutable state.
     */
    let mut older_accounts = newer_accounts.clone();

    older_accounts.observed_slot = 1_500;
    older_accounts.organization.paused = false;

    older_accounts.owner_member.authorized_wallet = SIGNER_WALLET.to_owned();
    older_accounts.owner_member.authorization_revision = 1;
    older_accounts.owner_member.roles = 3;
    older_accounts.owner_member.active = true;

    older_accounts.owner_member_wallet.authorized_wallet = SIGNER_WALLET.to_owned();

    let worker_id = format!("stale-snapshot-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    let context = reconciliation_context(&fixture);

    settle_finalized_initialize_organization(&pool, &claimed_job, &context, 1_000, &older_accounts)
        .await?;

    let organization_projection = sqlx::query!(
        r#"
        SELECT
            paused,
            observed_slot
        FROM organization_chain_accounts
        WHERE organization_id = $1
          AND cluster = 'devnet'
          AND program_id = $2
        "#,
        &fixture.organization_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let member_projection = sqlx::query!(
        r#"
        SELECT
            authorized_wallet,
            authorization_revision,
            roles,
            active,
            observed_slot
        FROM member_chain_accounts
        WHERE organization_id = $1
          AND member_id = $2
          AND cluster = 'devnet'
          AND program_id = $3
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let member_wallet_projection = sqlx::query!(
        r#"
        SELECT
            authorized_wallet,
            observed_slot
        FROM member_wallet_chain_accounts
        WHERE organization_id = $1
          AND member_id = $2
          AND cluster = 'devnet'
          AND program_id = $3
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert!(organization_projection.paused);
    assert_eq!(organization_projection.observed_slot, 2_000);

    assert_eq!(member_projection.authorized_wallet, newer_wallet);
    assert_eq!(
        member_projection.authorization_revision,
        BigDecimal::from(2_u64)
    );
    assert_eq!(member_projection.roles, 7);
    assert!(!member_projection.active);
    assert_eq!(member_projection.observed_slot, 2_000);

    assert_eq!(member_wallet_projection.authorized_wallet, newer_wallet);
    assert_eq!(member_wallet_projection.observed_slot, 2_000);

    Ok(())
}

#[tokio::test]
async fn stale_worker_rolls_back_finalized_success_settlement() -> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("success-stale-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job(&pool, &fixture, &worker_id).await?;

    let replacement_worker_id = format!("replacement-worker:{}", Uuid::new_v4());

    sqlx::query!(
        r#"
        UPDATE jobs
        SET locked_by = $2
        WHERE job_id = $1
          AND status = 'running'
        "#,
        fixture.job_id,
        &replacement_worker_id,
    )
    .execute(&pool)
    .await?;

    let context = reconciliation_context(&fixture);

    let accounts = validated_accounts(&fixture, 900_000_100);

    let result = settle_finalized_initialize_organization(
        &pool,
        &claimed_job,
        &context,
        900_000_000,
        &accounts,
    )
    .await;

    assert!(matches!(
        result,
        Err(ReconciliationSettlementError::JobQueue(
            JobQueueError::LeaseLost { .. }
        ))
    ));

    let submission_status = sqlx::query_scalar!(
        r#"
        SELECT status
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let lifecycle = sqlx::query!(
        r#"
        SELECT
            organization.status AS organization_status,
            owner_member.lifecycle_status AS owner_member_status

        FROM organizations AS organization

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             organization.organization_id
         AND owner_member.member_id = $2

        WHERE organization.organization_id = $1
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let organization_projection_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM organization_chain_accounts
        WHERE organization_id = $1
        "#,
        &fixture.organization_id,
    )
    .fetch_one(&pool)
    .await?;

    let member_projection_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM member_chain_accounts
        WHERE organization_id = $1
        "#,
        &fixture.organization_id,
    )
    .fetch_one(&pool)
    .await?;

    let member_wallet_projection_count = sqlx::query_scalar!(
        r#"
            SELECT COUNT(*) AS "count!"
            FROM member_wallet_chain_accounts
            WHERE organization_id = $1
            "#,
        &fixture.organization_id,
    )
    .fetch_one(&pool)
    .await?;

    let history_count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) AS "count!"
        FROM transaction_submission_status_history
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(submission_status, "submitted");

    assert_eq!(lifecycle.organization_status, "provisioning");

    assert_eq!(lifecycle.owner_member_status, "provisioning");

    assert_eq!(organization_projection_count, 0);
    assert_eq!(member_projection_count, 0);
    assert_eq!(member_wallet_projection_count, 0);

    /*
     * Only the initial NULL -> submitted status entry remains.
     * The finalized transition was rolled back.
     */
    assert_eq!(history_count, 1);

    Ok(())
}

async fn claim_fixture_job_for_processor(
    pool: &PgPool,
    fixture: &TestFixture,
    worker_id: &str,
) -> Result<ClaimedJob, Box<dyn Error>> {
    /*
     * Claim this exact fixture by job_id.
     *
     * We cannot use claim_next_job with the real reconciliation job type
     * because the development database may contain other queued
     * reconciliation jobs.
     */
    let claimed_job = sqlx::query_as!(
        ClaimedJob,
        r#"
        UPDATE jobs

        SET
            job_type = $2,
            status = 'running',
            attempts = attempts + 1,
            locked_at = now(),
            locked_by = $3,
            lease_expires_at =
                now() + INTERVAL '30 seconds'

        WHERE job_id = $1
          AND status = 'queued'

        RETURNING
            job_id,
            organization_id,
            submission_id,
            job_type,
            payload,
            idempotency_key,
            attempts,
            max_attempts,
            locked_at AS "locked_at!",
            locked_by AS "locked_by!",
            lease_expires_at AS "lease_expires_at!"
        "#,
        fixture.job_id,
        RECONCILIATION_JOB_TYPE,
        worker_id,
    )
    .fetch_one(pool)
    .await?;

    assert_eq!(claimed_job.job_id, fixture.job_id);

    assert_eq!(claimed_job.submission_id, Some(fixture.submission_id));

    assert_eq!(claimed_job.job_type, RECONCILIATION_JOB_TYPE);

    Ok(claimed_job)
}

#[tokio::test]
async fn processor_defers_confirmed_transaction_without_fetching_full_transaction()
-> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("processor-confirmed-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job_for_processor(&pool, &fixture, &worker_id).await?;

    let transaction_slot = 900_000_000;

    let rpc = FakeRpc::confirmed(transaction_slot);

    let result = process_claimed_initialize_organization_job(
        &pool,
        &rpc,
        "devnet",
        PROGRAM_ID,
        &claimed_job,
        60,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            locked_at,
            locked_by,
            lease_expires_at
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let calls = rpc.calls();

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(
        result,
        InitializeOrganizationProcessingResult::Deferred {
            submission_status: TransactionSubmissionStatus::Confirmed,
        }
    );

    assert_eq!(stored_submission.status, "confirmed");

    assert_eq!(
        stored_submission.confirmed_slot,
        Some(transaction_slot as i64)
    );

    assert_eq!(
        stored_submission.observed_commitment.as_deref(),
        Some("confirmed")
    );

    assert_eq!(stored_job.status, "queued");
    assert!(stored_job.locked_at.is_none());
    assert!(stored_job.locked_by.is_none());
    assert!(stored_job.lease_expires_at.is_none());

    assert_eq!(
        calls,
        FakeRpcCalls {
            signature_status: 1,
            finalized_block_height: 0,
            finalized_transaction: 0,
            finalized_accounts: 0,
        }
    );

    Ok(())
}

#[tokio::test]
async fn processor_expires_missing_transaction_after_last_valid_block_height()
-> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("processor-expiry-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job_for_processor(&pool, &fixture, &worker_id).await?;

    /*
     * The fixture's last valid block height is 1,000,000,000.
     *
     * Expiration occurs only when the finalized chain height is
     * strictly greater than that value.
     */
    let rpc = FakeRpc::missing(1_000_000_001);

    let result = process_claimed_initialize_organization_job(
        &pool,
        &rpc,
        "devnet",
        PROGRAM_ID,
        &claimed_job,
        60,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            error_code,
            failed_at
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let lifecycle = sqlx::query!(
        r#"
        SELECT
            organization.status AS organization_status,
            owner_member.lifecycle_status
                AS owner_member_status

        FROM organizations AS organization

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             organization.organization_id
         AND owner_member.member_id = $2

        WHERE organization.organization_id = $1
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            completed_at,
            locked_by
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let calls = rpc.calls();

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(result, InitializeOrganizationProcessingResult::Expired);

    assert_eq!(stored_submission.status, "expired");

    assert_eq!(
        stored_submission.error_code.as_deref(),
        Some("transaction_expired")
    );

    assert!(stored_submission.failed_at.is_some());

    assert_eq!(lifecycle.organization_status, "draft");
    assert_eq!(lifecycle.owner_member_status, "draft");

    /*
     * Reconciliation itself succeeded: it discovered and persisted the
     * terminal expired outcome.
     */
    assert_eq!(stored_job.status, "succeeded");
    assert!(stored_job.completed_at.is_some());
    assert!(stored_job.locked_by.is_none());

    assert_eq!(
        calls,
        FakeRpcCalls {
            signature_status: 1,
            finalized_block_height: 1,
            finalized_transaction: 0,
            finalized_accounts: 0,
        }
    );

    Ok(())
}

#[tokio::test]
async fn processor_validates_finalized_transaction_and_persists_all_projections()
-> Result<(), Box<dyn Error>> {
    let pool = connect_test_pool().await?;
    let fixture = insert_test_fixture(&pool).await?;

    let worker_id = format!("processor-finalized-worker:{}", Uuid::new_v4());

    let claimed_job = claim_fixture_job_for_processor(&pool, &fixture, &worker_id).await?;

    let organization_id = fixture.organization_id.to_string().parse::<u64>()?;

    let owner_member_id = fixture.owner_member_id.to_string().parse::<u64>()?;

    let pdas = derive_initialize_organization_pdas(
        PROGRAM_ID,
        SIGNER_WALLET,
        organization_id,
        owner_member_id,
    )?;

    let transaction_slot = 900_000_000;
    let account_observed_slot = 900_000_100;
    let block_time = 1_800_000_000;

    let instruction_data =
        initialize_organization_instruction_data(organization_id, owner_member_id);

    let transaction = FinalizedTransaction {
        slot: transaction_slot,
        block_time: Some(block_time),
        error: None,

        signatures: vec![fixture.transaction_signature.clone()],

        signer_addresses: vec![SIGNER_WALLET.to_owned()],

        instructions: vec![TransactionInstruction::PartiallyDecoded {
            program_id: PROGRAM_ID.to_owned(),

            accounts: vec![
                SIGNER_WALLET.to_owned(),
                pdas.organization_pda.clone(),
                pdas.owner_member_pda.clone(),
                pdas.owner_member_wallet_pda.clone(),
                SYSTEM_PROGRAM_ID.to_owned(),
            ],

            data: instruction_data,
        }],
    };

    let addresses = vec![
        pdas.organization_pda.clone(),
        pdas.owner_member_pda.clone(),
        pdas.owner_member_wallet_pda.clone(),
    ];

    let accounts = FinalizedAccountsLookup {
        context_slot: account_observed_slot,

        accounts: vec![
            Some(FinalizedAccountSnapshot {
                address: pdas.organization_pda.clone(),
                owner: PROGRAM_ID.to_owned(),
                lamports: 1_000_000,
                executable: false,

                data: organization_account_data(
                    organization_id,
                    SIGNER_WALLET,
                    &pdas.owner_member_pda,
                    pdas.organization_bump,
                ),
            }),
            Some(FinalizedAccountSnapshot {
                address: pdas.owner_member_pda.clone(),
                owner: PROGRAM_ID.to_owned(),
                lamports: 1_000_000,
                executable: false,

                data: member_account_data(
                    &pdas.organization_pda,
                    owner_member_id,
                    SIGNER_WALLET,
                    pdas.owner_member_bump,
                ),
            }),
            Some(FinalizedAccountSnapshot {
                address: pdas.owner_member_wallet_pda.clone(),

                owner: PROGRAM_ID.to_owned(),
                lamports: 1_000_000,
                executable: false,

                data: member_wallet_account_data(
                    &pdas.organization_pda,
                    &pdas.owner_member_pda,
                    SIGNER_WALLET,
                    pdas.owner_member_wallet_bump,
                ),
            }),
        ],
    };

    let rpc = FinalizedSuccessRpc {
        expected_signature: fixture.transaction_signature.clone(),

        expected_addresses: addresses,

        expected_min_context_slot: transaction_slot,

        signature_lookup: SignatureStatusLookup {
            context_slot: account_observed_slot,

            found_status: Some(FoundSignatureStatus {
                transaction_slot,
                confirmation_status: ConfirmationStatus::Finalized,
                error: None,
            }),
        },

        transaction,
        accounts,

        calls: Mutex::new(FakeRpcCalls::default()),
    };

    let result = process_claimed_initialize_organization_job(
        &pool,
        &rpc,
        "devnet",
        PROGRAM_ID,
        &claimed_job,
        60,
    )
    .await?;

    let stored_submission = sqlx::query!(
        r#"
        SELECT
            status,
            confirmed_slot,
            observed_commitment,
            finalized_at,
            error_code
        FROM transaction_submissions
        WHERE submission_id = $1
        "#,
        fixture.submission_id,
    )
    .fetch_one(&pool)
    .await?;

    let lifecycle = sqlx::query!(
        r#"
        SELECT
            organization.status AS organization_status,
            owner_member.lifecycle_status
                AS owner_member_status

        FROM organizations AS organization

        JOIN organization_members AS owner_member
          ON owner_member.organization_id =
             organization.organization_id
         AND owner_member.member_id = $2

        WHERE organization.organization_id = $1
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
    )
    .fetch_one(&pool)
    .await?;

    let organization_projection = sqlx::query!(
        r#"
        SELECT
            organization_pda,
            owner_member_pda,
            observed_slot
        FROM organization_chain_accounts
        WHERE organization_id = $1
          AND cluster = 'devnet'
          AND program_id = $2
        "#,
        &fixture.organization_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let member_projection = sqlx::query!(
        r#"
        SELECT
            member_pda,
            organization_pda,
            observed_slot
        FROM member_chain_accounts
        WHERE organization_id = $1
          AND member_id = $2
          AND cluster = 'devnet'
          AND program_id = $3
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let member_wallet_projection = sqlx::query!(
        r#"
        SELECT
            member_wallet_pda,
            member_pda,
            organization_pda,
            observed_slot
        FROM member_wallet_chain_accounts
        WHERE organization_id = $1
          AND member_id = $2
          AND cluster = 'devnet'
          AND program_id = $3
        "#,
        &fixture.organization_id,
        &fixture.owner_member_id,
        PROGRAM_ID,
    )
    .fetch_one(&pool)
    .await?;

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            completed_at,
            locked_by
        FROM jobs
        WHERE job_id = $1
        "#,
        fixture.job_id,
    )
    .fetch_one(&pool)
    .await?;

    let calls = rpc.calls();

    cleanup_fixture(&pool, &fixture).await?;

    assert_eq!(
        result,
        InitializeOrganizationProcessingResult::FinalizedSuccess {
            transaction_slot,
            account_observed_slot,
            block_time: Some(block_time),
        }
    );

    assert_eq!(stored_submission.status, "finalized");

    assert_eq!(
        stored_submission.confirmed_slot,
        Some(transaction_slot as i64)
    );

    assert_eq!(
        stored_submission.observed_commitment.as_deref(),
        Some("finalized")
    );

    assert!(stored_submission.finalized_at.is_some());
    assert!(stored_submission.error_code.is_none());

    assert_eq!(lifecycle.organization_status, "active");
    assert_eq!(lifecycle.owner_member_status, "active");

    assert_eq!(
        organization_projection.organization_pda,
        pdas.organization_pda
    );

    assert_eq!(
        organization_projection.owner_member_pda,
        pdas.owner_member_pda
    );

    assert_eq!(
        organization_projection.observed_slot,
        account_observed_slot as i64
    );

    assert_eq!(member_projection.member_pda, pdas.owner_member_pda);

    assert_eq!(member_projection.organization_pda, pdas.organization_pda);

    assert_eq!(
        member_projection.observed_slot,
        account_observed_slot as i64
    );

    assert_eq!(
        member_wallet_projection.member_wallet_pda,
        pdas.owner_member_wallet_pda
    );

    assert_eq!(member_wallet_projection.member_pda, pdas.owner_member_pda);

    assert_eq!(
        member_wallet_projection.organization_pda,
        pdas.organization_pda
    );

    assert_eq!(
        member_wallet_projection.observed_slot,
        account_observed_slot as i64
    );

    assert_eq!(stored_job.status, "succeeded");
    assert!(stored_job.completed_at.is_some());
    assert!(stored_job.locked_by.is_none());

    assert_eq!(
        calls,
        FakeRpcCalls {
            signature_status: 1,
            finalized_block_height: 0,
            finalized_transaction: 1,
            finalized_accounts: 1,
        }
    );

    Ok(())
}
