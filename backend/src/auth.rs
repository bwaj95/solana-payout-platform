use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::{StatusCode, header::AUTHORIZATION, request::Parts},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{AppState, error::AppError};

const CHALLENGE_LIFETIME_MINUTES: i64 = 5;
const SESSION_LIFETIME_HOURS: i64 = 8;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateChallengeRequest {
    wallet_address: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ChallengeResponse {
    challenge_id: Uuid,
    wallet_address: String,
    message: String,
    expires_at: DateTime<Utc>,
}

struct WalletAuthChallengeRow {
    challenge_id: Uuid,
    wallet_address: String,
    message: String,
    expires_at: DateTime<Utc>,
}

impl From<WalletAuthChallengeRow> for ChallengeResponse {
    fn from(value: WalletAuthChallengeRow) -> Self {
        Self {
            challenge_id: value.challenge_id,
            wallet_address: value.wallet_address,
            message: value.message,
            expires_at: value.expires_at,
        }
    }
}

struct WalletAuthChallengeForVerificationRow {
    wallet_address: String,
    message: String,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VerifyChallengeRequest {
    challenge_id: Uuid,
    wallet_address: String,

    /*
     * The frontend Base58-encodes the 64 signature bytes returned by
     * wallet.signMessage().
     */
    signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    session_id: Uuid,
    wallet_address: String,
    access_token: String,
    token_type: &'static str,
    expires_at: DateTime<Utc>,
}

struct ActiveSessionRow {
    session_id: Uuid,
    wallet_address: String,
    expires_at: DateTime<Utc>,
}

/*
 * This becomes a handler argument for protected endpoints.
 *
 * Its presence means the bearer token has already been validated against
 * an active, unexpired and non-revoked database session.
 */
#[derive(Debug, Clone)]
pub(crate) struct AuthenticatedWallet {
    pub session_id: Uuid,
    pub wallet_address: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CurrentSessionResponse {
    session_id: Uuid,
    wallet_address: String,
    expires_at: DateTime<Utc>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/challenge", post(create_challenge))
        .route("/api/v1/auth/verify", post(verify_challenge))
        .route("/api/v1/auth/me", get(get_current_session))
}

async fn create_challenge(
    State(state): State<AppState>,
    Json(request): Json<CreateChallengeRequest>,
) -> Result<(StatusCode, Json<ChallengeResponse>), AppError> {
    let wallet_address = request.wallet_address.trim().to_owned();

    validate_wallet_address(&wallet_address)?;

    let challenge_id = Uuid::new_v4();
    let issued_at = Utc::now();
    let expires_at = issued_at + Duration::minutes(CHALLENGE_LIFETIME_MINUTES);

    /*
     * The frontend must ask the wallet to sign this exact message.
     *
     * Because the nonce and expiry are included in the signed message,
     * an old signature cannot be reused for a newly created challenge.
     */
    let message = format!(
        "\
Solana Payout Platform wants you to authenticate.

Wallet: {wallet_address}
Network: Solana Devnet
Challenge: {challenge_id}
Issued At: {}
Expiration Time: {}

Signing this message will not submit a transaction or cost any SOL.",
        issued_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        expires_at.to_rfc3339_opts(SecondsFormat::Millis, true),
    );

    let row: WalletAuthChallengeRow = insert_challenge(
        &state.db,
        challenge_id,
        &wallet_address,
        &message,
        expires_at,
    )
    .await?;

    let response: ChallengeResponse = ChallengeResponse::from(row);

    Ok((StatusCode::CREATED, Json(response)))
}

async fn verify_challenge(
    State(state): State<AppState>,
    Json(request): Json<VerifyChallengeRequest>,
) -> Result<Json<SessionResponse>, AppError> {
    let wallet_address = request.wallet_address.trim().to_owned();

    validate_wallet_address(&wallet_address)?;

    /*
     * This database transaction makes challenge consumption and session
     * creation atomic.
     *
     * If two requests try to reuse the same signature concurrently,
     * SELECT ... FOR UPDATE allows only one request to consume the challenge.
     */
    let mut transaction = state.db.begin().await?;

    let challenge: WalletAuthChallengeForVerificationRow = sqlx::query_as!(
        WalletAuthChallengeForVerificationRow,
        r#"
            SELECT
                wallet_address,
                message,
                expires_at,
                consumed_at
            FROM wallet_auth_challenges
            WHERE challenge_id = $1
            FOR UPDATE
            "#,
        request.challenge_id,
    )
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or(AppError::AuthenticationChallengeNotFound)?;

    if challenge.wallet_address != wallet_address {
        return Err(AppError::AuthenticationWalletMismatch);
    }

    // checks if challenge.consumed_at is not null
    if challenge.consumed_at.is_some() {
        return Err(AppError::AuthenticationChallengeConsumed);
    }

    let now = Utc::now();

    if challenge.expires_at <= now {
        return Err(AppError::AuthenticationChallengeExpired);
    }

    verify_wallet_signature(&wallet_address, &challenge.message, &request.signature)?;

    /*
     * The raw access token is returned to the client exactly once.
     *
     * Only its SHA-256 hash is stored. Therefore, somebody reading the
     * database cannot copy a stored token and use it as a session.
     */
    let access_token_bytes: [u8; 32] = rand::random();
    let access_token = URL_SAFE_NO_PAD.encode(access_token_bytes);
    let token_hash = Sha256::digest(access_token.as_bytes()).to_vec();

    let session_id = Uuid::new_v4();
    let session_expires_at = now + Duration::hours(SESSION_LIFETIME_HOURS);

    sqlx::query!(
        r#"
        UPDATE wallet_auth_challenges
        SET consumed_at = $2
        WHERE challenge_id = $1
        "#,
        request.challenge_id,
        now,
    )
    .execute(&mut *transaction)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO wallet_sessions (
            session_id,
            token_hash,
            wallet_address,
            expires_at
        )
        VALUES ($1, $2, $3, $4)
        "#,
        session_id,
        token_hash,
        wallet_address,
        session_expires_at,
    )
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(Json(SessionResponse {
        session_id,
        wallet_address,
        access_token,
        token_type: "Bearer",
        expires_at: session_expires_at,
    }))
}

fn validate_wallet_address(wallet_address: &str) -> Result<(), AppError> {
    let decoded = bs58::decode(wallet_address)
        .into_vec()
        .map_err(|_| AppError::InvalidWalletAddress)?;

    /*
     * Every Solana public key contains exactly 32 bytes.
     *
     * Checking only the string length is insufficient because different
     * Base58 strings can contain different numbers of decoded bytes.
     */
    if decoded.len() != 32 {
        return Err(AppError::InvalidWalletAddress);
    }

    let canonical_address = bs58::encode(decoded).into_string();

    if canonical_address != wallet_address {
        return Err(AppError::InvalidWalletAddress);
    }

    Ok(())
}

fn verify_wallet_signature(
    wallet_address: &str,
    message: &str,
    encoded_signature: &str,
) -> Result<(), AppError> {
    let public_key_bytes = bs58::decode(wallet_address)
        .into_vec()
        .map_err(|_| AppError::InvalidWalletAddress)?;

    let public_key_bytes: [u8; 32] = public_key_bytes
        .try_into()
        .map_err(|_| AppError::InvalidWalletAddress)?;

    let verifying_key =
        VerifyingKey::from_bytes(&public_key_bytes).map_err(|_| AppError::InvalidWalletAddress)?;

    let signature_bytes = bs58::decode(encoded_signature)
        .into_vec()
        .map_err(|_| AppError::InvalidWalletSignature)?;

    let signature =
        Signature::from_slice(&signature_bytes).map_err(|_| AppError::InvalidWalletSignature)?;

    /*
     * Solana wallets use Ed25519.
     *
     * This proves that the signature was produced by the private key
     * corresponding to wallet_address, over these exact message bytes.
     */
    verifying_key
        .verify_strict(message.as_bytes(), &signature)
        .map_err(|_| AppError::InvalidWalletSignature)
}

async fn insert_challenge(
    pool: &PgPool,
    challenge_id: Uuid,
    wallet_address: &str,
    message: &str,
    expires_at: DateTime<Utc>,
) -> Result<WalletAuthChallengeRow, sqlx::Error> {
    sqlx::query_as!(
        WalletAuthChallengeRow,
        r#"
        INSERT INTO wallet_auth_challenges (
            challenge_id,
            wallet_address,
            message,
            expires_at
        )
        VALUES ($1, $2, $3, $4)
        RETURNING
            challenge_id,
            wallet_address,
            message,
            expires_at
        "#,
        challenge_id,
        wallet_address,
        message,
        expires_at,
    )
    .fetch_one(pool)
    .await
}

async fn get_current_session(
    authenticated_wallet: AuthenticatedWallet,
) -> Json<CurrentSessionResponse> {
    Json(CurrentSessionResponse {
        session_id: authenticated_wallet.session_id,
        wallet_address: authenticated_wallet.wallet_address,
        expires_at: authenticated_wallet.expires_at,
    })
}

impl FromRequestParts<AppState> for AuthenticatedWallet {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let authorization_header = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or(AppError::MissingAuthenticationToken)?;

        let authorization_header = authorization_header
            .to_str()
            .map_err(|_| AppError::InvalidAuthenticationToken)?;

        let (scheme, access_token) = authorization_header
            .split_once(' ')
            .ok_or(AppError::InvalidAuthenticationToken)?;

        if !scheme.eq_ignore_ascii_case("Bearer")
            || access_token.is_empty()
            || access_token.chars().any(char::is_whitespace)
        {
            return Err(AppError::InvalidAuthenticationToken);
        }

        /*
         * The database contains SHA-256(token), not the raw token.
         * Therefore, hash the received bearer token before querying.
         */
        let token_hash = Sha256::digest(access_token.as_bytes()).to_vec();

        let session = sqlx::query_as!(
            ActiveSessionRow,
            r#"
            SELECT
                session_id,
                wallet_address,
                expires_at
            FROM wallet_sessions
            WHERE token_hash = $1
              AND revoked_at IS NULL
              AND expires_at > now()
            "#,
            token_hash,
        )
        .fetch_optional(&state.db)
        .await?
        .ok_or(AppError::InvalidAuthenticationToken)?;

        Ok(Self {
            session_id: session.session_id,
            wallet_address: session.wallet_address,
            expires_at: session.expires_at,
        })
    }
}
