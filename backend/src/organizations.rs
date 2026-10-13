use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use bigdecimal::BigDecimal;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{AppState, error::AppError};

#[derive(Debug)]
pub struct OrganizationRow {
    organization_id: BigDecimal,
    name: String,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Serialize)] // waht are these
#[serde(rename_all = "camelCase")]
pub struct OrganizationResponse {
    organization_id: String,
    name: String,
    status: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrganizationRequest {
    name: String,
}

// FromRow: database row → internal Rust database struct.
// From: internal Rust struct → API response struct.
// Serialize: API response struct → JSON.
impl From<OrganizationRow> for OrganizationResponse {
    fn from(row: OrganizationRow) -> Self {
        Self {
            organization_id: row.organization_id.to_string(),
            name: row.name,
            status: row.status,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug)]
pub struct OrganizationChainAccountRow {
    cluster: String,
    program_id: String,
    organization_pda: String,
    creator_wallet: String,
    owner_member_pda: String,
    paused: bool,
    bump: i16, //bump uses i16 because PostgreSQL has no unsigned u8; the database constraint still guarantees 0..=255
    observed_slot: i64,
    observed_commitment: String,
    first_observed_at: DateTime<Utc>,
    last_synced_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationChainAccountResponse {
    cluster: String,
    program_id: String,
    organization_pda: String,
    creator_wallet: String,
    owner_member_pda: String,
    paused: bool,
    bump: i16,
    observed_slot: i64,
    observed_commitment: String,
    first_observed_at: DateTime<Utc>,
    last_synced_at: DateTime<Utc>,
}

impl From<OrganizationChainAccountRow> for OrganizationChainAccountResponse {
    fn from(row: OrganizationChainAccountRow) -> Self {
        Self {
            cluster: row.cluster,
            program_id: row.program_id,
            organization_pda: row.organization_pda,
            creator_wallet: row.creator_wallet,
            owner_member_pda: row.owner_member_pda,
            paused: row.paused,
            bump: row.bump,
            observed_slot: row.observed_slot,
            observed_commitment: row.observed_commitment,
            first_observed_at: row.first_observed_at,
            last_synced_at: row.last_synced_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationDetailResponse {
    organization: OrganizationResponse,
    chain_accounts: Vec<OrganizationChainAccountResponse>,
}

// organizations router
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/organizations",
            get(list_organizations).post(create_organization),
        )
        .route(
            "/api/v1/organizations/{organization_id}",
            get(get_organization), // error here
        )
}

async fn list_organizations(
    State(state): State<AppState>,
) -> Result<Json<Vec<OrganizationResponse>>, AppError> {
    let rows = fetch_organizations(&state.db).await?;

    let organizations = rows.into_iter().map(OrganizationResponse::from).collect();

    Ok(Json(organizations))
}

async fn create_organization(
    State(state): State<AppState>,
    Json(request): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), AppError> {
    let name = request.name.trim();

    if name.is_empty() || name.chars().count() > 120 {
        return Err(AppError::InvalidOrganizationName);
    }

    let row = insert_organization(&state.db, name).await?;

    Ok((StatusCode::CREATED, Json(OrganizationResponse::from(row))))
}

async fn get_organization(
    State(state): State<AppState>,
    Path(organization_id): Path<u64>,
) -> Result<Json<OrganizationDetailResponse>, AppError> {
    let organization = fetch_organization(&state.db, organization_id)
        .await?
        .ok_or(AppError::OrganizationNotFound)?;

    let chain_accounts = fetch_organization_chain_accounts(&state.db, organization_id).await?;

    let chain_accounts_response = chain_accounts
        .into_iter()
        .map(OrganizationChainAccountResponse::from)
        .collect();

    let response = OrganizationDetailResponse {
        organization: OrganizationResponse::from(organization),
        chain_accounts: chain_accounts_response,
    };

    Ok(Json(response))
}

// Database Functions
async fn fetch_organizations(pool: &PgPool) -> Result<Vec<OrganizationRow>, sqlx::Error> {
    sqlx::query_as!(
        OrganizationRow,
        r#"
        SELECT
            organization_id,
            name,
            status,
            created_at,
            updated_at
        FROM organizations
        ORDER BY organization_id
        "#,
    )
    .fetch_all(pool)
    .await
}

async fn insert_organization(pool: &PgPool, name: &str) -> Result<OrganizationRow, sqlx::Error> {
    sqlx::query_as!(
        OrganizationRow,
        r#"
        INSERT INTO organizations (
            name,
            status
        )
        VALUES (
            $1,
            'draft'
        )
        RETURNING
            organization_id,
            name,
            status,
            created_at,
            updated_at
        "#,
        name,
    )
    .fetch_one(pool)
    .await
}

async fn fetch_organization(
    pool: &PgPool,
    organization_id: u64,
) -> Result<Option<OrganizationRow>, sqlx::Error> {
    let organization_id = BigDecimal::from(organization_id);

    sqlx::query_as!(
        OrganizationRow,
        r#"
        SELECT
            organization_id,
            name,
            status,
            created_at,
            updated_at
        FROM organizations
        WHERE organization_id = $1
        "#,
        organization_id,
    )
    .fetch_optional(pool)
    .await
}

async fn fetch_organization_chain_accounts(
    pool: &PgPool,
    organization_id: u64,
) -> Result<Vec<OrganizationChainAccountRow>, sqlx::Error> {
    let organization_id = BigDecimal::from(organization_id);

    sqlx::query_as!(
        OrganizationChainAccountRow,
        r#"
        SELECT
            cluster,
            program_id,
            organization_pda,
            creator_wallet,
            owner_member_pda,
            paused,
            bump,
            observed_slot,
            observed_commitment,
            first_observed_at,
            last_synced_at
        FROM organization_chain_accounts
        WHERE organization_id = $1
        ORDER BY cluster, program_id
        "#,
        organization_id,
    )
    .fetch_all(pool)
    .await
}
