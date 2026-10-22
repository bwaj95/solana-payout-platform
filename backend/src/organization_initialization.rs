use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    AppState,
    auth::AuthenticatedWallet,
    domain::{
        INITIALIZE_ORGANIZATION_INSTRUCTION, LifecycleStatus, RECONCILE_TRANSACTION_SUBMISSION_JOB,
        ROLE_OWNER, ReportSubmissionStatus,
    },
    error::AppError,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordInitializationSubmissionRequest {
    idempotency_key: Uuid,
    transaction_signature: String,
    last_valid_block_height: i64,
    submission_status: ReportSubmissionStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializationSubmissionResponse {
    submission_id: i64,
    idempotency_key: Uuid,

    organization_id: String,
    owner_member_id: String,

    organization_status: String,
    owner_member_status: String,

    transaction_signature: String,
    last_valid_block_height: i64,
    submission_status: String,

    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

struct OrganizationLifecycleRow {
    organization_id: BigDecimal,
    status: String,
}

struct OwnerLifecycleRow {
    member_id: BigDecimal,
    requested_authorized_wallet: String,
    lifecycle_status: String,
}

struct TransactionSubmissionRow {
    submission_id: i64,
    idempotency_key: Uuid,

    organization_id: BigDecimal,
    signer_wallet: String,
    cluster: String,
    program_id: String,
    instruction_name: String,

    transaction_signature: Option<String>,
    last_valid_block_height: Option<i64>,
    status: String,

    entity_key: Value,
    request_payload: Value,

    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/organizations/{organization_id}/initialization-submissions",
        post(record_initialization_submission),
    )
}

async fn record_initialization_submission(
    authenticated_wallet: AuthenticatedWallet,
    State(state): State<AppState>,
    Path(organization_id): Path<u64>,
    Json(request): Json<RecordInitializationSubmissionRequest>,
) -> Result<(StatusCode, Json<InitializationSubmissionResponse>), AppError> {
    let transaction_signature = request.transaction_signature.trim().to_owned();

    validate_transaction_signature(&transaction_signature)?;

    if request.last_valid_block_height < 0 {
        return Err(AppError::InvalidLastValidBlockHeight);
    }

    let organization_id = BigDecimal::from(organization_id);
    let cluster = state.solana.cluster.clone();
    let program_id = state.solana.program_id.clone();
    let signer_wallet = authenticated_wallet.wallet_address;
    let idempotency_key = request.idempotency_key;
    let last_valid_block_height = request.last_valid_block_height;
    let submission_status = request.submission_status;

    let mut transaction = state.db.begin().await?;

    let organization = lock_organization(&mut transaction, &organization_id)
        .await?
        .ok_or(AppError::OrganizationNotFound)?;

    let owner_member = lock_owner_member(&mut transaction, &organization_id)
        .await?
        .ok_or(AppError::OrganizationOwnerNotFound)?;

    if owner_member.requested_authorized_wallet != signer_wallet {
        return Err(AppError::OrganizationAccessDenied);
    }

    let organization_id_string = organization.organization_id.to_string();
    let owner_member_id_string = owner_member.member_id.to_string();

    let entity_key = json!({
        "organization_id": organization_id_string.clone(),
    });

    let request_payload = json!({
        "organization_id": organization_id_string,
        "owner_member_id": owner_member_id_string,
    });

    /*
     * Check idempotency before checking lifecycle status.
     *
     * This allows a retry of the same request to return its original
     * submission even though the Organization is now provisioning or active.
     */
    if let Some(existing) =
        find_submission_by_idempotency_key(&mut transaction, &idempotency_key).await?
    {
        validate_idempotent_retry(
            &existing,
            &organization_id,
            &cluster,
            &program_id,
            &signer_wallet,
            &entity_key,
            &request_payload,
            &transaction_signature,
            last_valid_block_height,
        )?;

        let response = build_initialization_submission_response(
            existing,
            &owner_member.member_id,
            &transaction_signature,
            last_valid_block_height,
            &organization.status,
            &owner_member.lifecycle_status,
        );

        transaction.commit().await?;

        return Ok((StatusCode::ACCEPTED, Json(response)));
    }

    // check current status still draft, so that we can go ahead and process
    if organization.status != LifecycleStatus::Draft.as_str()
        || owner_member.lifecycle_status != LifecycleStatus::Draft.as_str()
    {
        return Err(AppError::OrganizationInitializationNotAllowed);
    }

    // Record the frontend's Solana transaction attempt in PostgreSQL - ignore on conflict to prevent reuse of idempotency key and the tx signature,
    let submission_result = insert_transaction_submission(
        &mut transaction,
        idempotency_key,
        &organization_id,
        &cluster,
        &program_id,
        &signer_wallet,
        &entity_key,
        &request_payload,
        &transaction_signature,
        last_valid_block_height,
        submission_status.as_str(),
    )
    .await?;

    /*
     * ON CONFLICT protects against concurrent reuse of either the
     * idempotency key or transaction signature.
     */
    let submission = match submission_result {
        Some(submission) => submission,

        None => {
            if let Some(existing) =
                find_submission_by_idempotency_key(&mut transaction, &idempotency_key).await?
            {
                validate_idempotent_retry(
                    &existing,
                    &organization_id,
                    &cluster,
                    &program_id,
                    &signer_wallet,
                    &entity_key,
                    &request_payload,
                    &transaction_signature,
                    last_valid_block_height,
                )?;

                let response = build_initialization_submission_response(
                    existing,
                    &owner_member.member_id,
                    &transaction_signature,
                    last_valid_block_height,
                    &organization.status,
                    &owner_member.lifecycle_status,
                );

                transaction.commit().await?;

                return Ok((StatusCode::ACCEPTED, Json(response)));
            }

            return Err(AppError::TransactionSignatureAlreadyRecorded);
        }
    };

    // the transaction submission has been made at this point

    // mark org and owner as provisioning
    mark_organization_provisioning(&mut transaction, &organization_id, &owner_member.member_id)
        .await?;

    // enque tx reconcilation job
    enqueue_reconciliation_job(&mut transaction, &organization_id, submission.submission_id)
        .await?;

    let response = build_initialization_submission_response(
        submission,
        &owner_member.member_id,
        &transaction_signature,
        last_valid_block_height,
        LifecycleStatus::Provisioning.as_str(),
        LifecycleStatus::Provisioning.as_str(),
    );

    // commit tx
    transaction.commit().await?;

    // return response with details.

    Ok((StatusCode::ACCEPTED, Json(response)))
}

pub fn validate_transaction_signature(transaction_signature: &str) -> Result<(), AppError> {
    let decoded = bs58::decode(transaction_signature)
        .into_vec()
        .map_err(|_| AppError::InvalidTransactionSignature)?;

    // Decoded Solana Ed25519 signatures are 64 bytes long
    if decoded.len() != 64 {
        return Err(AppError::InvalidTransactionSignature);
    }

    Ok(())
}

fn validate_idempotent_retry(
    existing: &TransactionSubmissionRow,
    organization_id: &BigDecimal,
    cluster: &str,
    program_id: &str,
    signer_wallet: &str,
    entity_key: &Value,
    request_payload: &Value,
    transaction_signature: &str,
    last_valid_block_height: i64,
) -> Result<(), AppError> {
    let matches_original_request = existing.organization_id == *organization_id
        && existing.cluster == cluster
        && existing.program_id == program_id
        && existing.instruction_name == INITIALIZE_ORGANIZATION_INSTRUCTION
        && existing.signer_wallet == signer_wallet
        && existing.entity_key == *entity_key
        && existing.request_payload == *request_payload
        && existing.transaction_signature.as_deref() == Some(transaction_signature) // why the deref?
        && existing.last_valid_block_height == Some(last_valid_block_height); // and why the Some() in both places?

    if !matches_original_request {
        return Err(AppError::IdempotencyKeyConflict);
    }

    Ok(())
}

fn build_initialization_submission_response(
    submission: TransactionSubmissionRow,
    owner_member_id: &BigDecimal,
    transaction_signature: &str,
    last_valid_block_height: i64,
    organization_status: &str,
    owner_member_status: &str,
) -> InitializationSubmissionResponse {
    InitializationSubmissionResponse {
        submission_id: submission.submission_id,
        idempotency_key: submission.idempotency_key,

        organization_id: submission.organization_id.to_string(),
        owner_member_id: owner_member_id.to_string(),

        transaction_signature: transaction_signature.to_owned(),
        last_valid_block_height,
        submission_status: submission.status,

        organization_status: organization_status.to_owned(), // why to_owned() here?
        owner_member_status: owner_member_status.to_owned(),

        created_at: submission.created_at,
        updated_at: submission.updated_at,
    }
}

// Database calls
async fn lock_organization(
    connection: &mut PgConnection,
    organization_id: &BigDecimal,
) -> Result<Option<OrganizationLifecycleRow>, sqlx::Error> {
    sqlx::query_as!(
        OrganizationLifecycleRow,
        r#"
        SELECT 
            organization_id, status
        FROM 
            organizations
        WHERE 
            organization_id = $1
        FOR UPDATE
        "#,
        organization_id
    )
    .fetch_optional(connection)
    .await
}

async fn lock_owner_member(
    connection: &mut PgConnection,
    organization_id: &BigDecimal,
) -> Result<Option<OwnerLifecycleRow>, sqlx::Error> {
    sqlx::query_as!(
        OwnerLifecycleRow,
        r#"
        SELECT member_id, requested_authorized_wallet, lifecycle_status
        FROM organization_members
        WHERE organization_id = $1 AND (requested_roles & $2) <> 0
        FOR UPDATE
        "#,
        organization_id,
        ROLE_OWNER
    )
    .fetch_optional(connection)
    .await
}

async fn find_submission_by_idempotency_key(
    connection: &mut PgConnection,
    idempotency_key: &Uuid,
) -> Result<Option<TransactionSubmissionRow>, sqlx::Error> {
    sqlx::query_as!(
        TransactionSubmissionRow,
        r#"
        SELECT 
            submission_id,
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
            created_at,
            updated_at
        FROM transaction_submissions
        WHERE idempotency_key = $1
        "#,
        idempotency_key
    )
    .fetch_optional(connection)
    .await
}

#[allow(clippy::too_many_arguments)]
async fn insert_transaction_submission(
    connection: &mut PgConnection,
    idempotency_key: Uuid,
    organization_id: &BigDecimal,
    cluster: &str,
    program_id: &str,
    signer_wallet: &str,
    entity_key: &Value,
    request_payload: &Value,
    transaction_signature: &str,
    last_valid_block_height: i64,
    status: &str,
) -> Result<Option<TransactionSubmissionRow>, sqlx::Error> {
    sqlx::query_as!(
        TransactionSubmissionRow,
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
            $3,
            $4,
            $5,
            $6,
            $7,
            $8,
            $9,
            $10,
            $11,
            now()
        )
        ON CONFLICT DO NOTHING
        RETURNING
            submission_id,
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
            created_at,
            updated_at
        "#,
        idempotency_key,
        organization_id,
        cluster,
        program_id,
        INITIALIZE_ORGANIZATION_INSTRUCTION,
        signer_wallet,
        entity_key,
        request_payload,
        transaction_signature,
        last_valid_block_height,
        status,
    )
    .fetch_optional(connection)
    .await
}

async fn mark_organization_provisioning(
    connection: &mut PgConnection,
    organization_id: &BigDecimal,
    owner_member_id: &BigDecimal,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE organizations
        SET status = $2
        WHERE organization_id = $1
        "#,
        organization_id,
        LifecycleStatus::Provisioning.as_str()
    )
    .execute(&mut *connection)
    .await?;

    sqlx::query!(
        r#"
        UPDATE organization_members
        SET lifecycle_status = $3
        WHERE organization_id = $1 AND member_id = $2
        "#,
        organization_id,
        owner_member_id,
        LifecycleStatus::Provisioning.as_str()
    )
    .execute(&mut *connection)
    .await?;

    Ok(())
}

async fn enqueue_reconciliation_job(
    connection: &mut PgConnection,
    organization_id: &BigDecimal,
    submission_id: i64,
) -> Result<(), sqlx::Error> {
    let payload = json!({
        "submission_id": submission_id,
        "organization_id": organization_id.to_string(),
    });

    let job_idempotency_key = format!("reconcile-transaction-submission:{submission_id}");

    sqlx::query!(
        r#"
        INSERT INTO jobs (
            organization_id,
            submission_id,
            job_type,
            payload,
            idempotency_key
        )
        VALUES (
            $1,
            $2,
            $3,
            $4,
            $5
        )
        "#,
        organization_id,
        submission_id,
        RECONCILE_TRANSACTION_SUBMISSION_JOB,
        payload,
        job_idempotency_key,
    )
    .execute(connection)
    .await?;

    Ok(())
}
