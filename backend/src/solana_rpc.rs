use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use thiserror::Error;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

#[derive(Clone)]
pub struct SolanaRpcClient {
    http_client: reqwest::Client,
    endpoint: reqwest::Url,

    next_request_id: Arc<AtomicU64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmationStatus {
    Processed,
    Confirmed,
    Finalized,
}

/*
 * Transaction-specific status returned when the signature was found.
 * `transaction_slot` is the slot in which the transaction was processed.
 * The confirmation status and optional error describe its current outcome.
 */
#[derive(Debug, Clone, PartialEq)]
pub struct FoundSignatureStatus {
    pub transaction_slot: u64,
    pub confirmation_status: ConfirmationStatus,

    /*
     * None means the transaction has not reported an execution error.
     *
     * Some(JSON) contains Solana's structured transaction error. We keep
     * it as JSON because transaction errors can have several shapes.
     */
    pub error: Option<Value>,
}

/*
 * Complete result of looking up one transaction signature.
 * `context_slot` identifies the RPC node's chain context when it answered.
 * denotes how fresh the rpc data was when it served this request
 *
 * `status` is `None` when the node did not find the signature.
 */
#[derive(Debug, Clone, PartialEq)]
pub struct SignatureStatusLookup {
    pub context_slot: u64,
    pub found_status: Option<FoundSignatureStatus>,
}

/*
 * Finalized transaction data normalized for the reconciliation layer.
 *
 * Raw RPC-specific wrappers and field names remain private to this module.
 */
#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedTransaction {
    pub slot: u64,
    pub block_time: Option<i64>,
    pub error: Option<Value>,

    // first signature is the transaction ID.
    pub signatures: Vec<String>,

    // all signers that signed the transaction
    pub signer_addresses: Vec<String>,

    pub instructions: Vec<TransactionInstruction>,
}

/*
 * With jsonParsed encoding, known native/program instructions may be fully
 * parsed while custom Anchor instructions remain partially decoded.
 */
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionInstruction {
    PartiallyDecoded {
        program_id: String,
        accounts: Vec<String>,
        data: Vec<u8>,
    },

    Parsed {
        program_id: String,
        parsed: Value,
    },
}

/*
 * One account snapshot returned from finalized Solana state.
 *
 * `address` is copied from the corresponding request position because
 * getMultipleAccounts returns account values in the same order as the
 * requested addresses.
 *
 * `data` contains the decoded raw account bytes. The account validation
 * layer will later decode the Anchor discriminator and account fields.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedAccountSnapshot {
    pub address: String,
    pub owner: String,
    pub lamports: u64,
    pub executable: bool,
    pub data: Vec<u8>,
}

/*
 * Result of reading several accounts from one finalized RPC context.
 *
 * An entry is None when the RPC node reports that the corresponding
 * address does not currently contain an account.
 */
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedAccountsLookup {
    pub context_slot: u64,
    pub accounts: Vec<Option<FinalizedAccountSnapshot>>,
}

#[derive(Debug, Error)]
pub enum SolanaRpcError {
    #[error("invalid Solana RPC URL: {0}")]
    InvalidRpcUrl(String),

    #[error("failed to construct Solana RPC HTTP client")]
    ClientConstruction(#[source] reqwest::Error),

    #[error("Solana RPC transport request failed")]
    Transport(#[source] reqwest::Error),

    #[error("Solana RPC returned HTTP status {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("failed to decode Solana RPC response")]
    ResponseDecode(#[source] serde_json::Error),

    #[error("Solana RPC returned error {code}: {message}; data: {data:?}")]
    Rpc {
        code: i64,
        message: String,
        data: Option<Value>,
    },

    #[error("Solana RPC response ID did not match request ID")]
    ResponseIdMismatch,

    #[error("invalid Solana RPC response: {0}")]
    InvalidResponse(String),

    #[error("Solana RPC returned instruction data that was not valid Base58")]
    InstructionDataDecode(#[source] bs58::decode::Error),

    #[error("failed to decode Base64 Solana account data")]
    AccountDataDecode(#[source] base64::DecodeError),
}

// private structs to process the direct rpc response.
// public structs serve as interface to other parts of the application.
// this way the private response handling structs dont leak across the entire application.
#[derive(Debug, Deserialize)]
struct JsonRpcEnvelope {
    jsonrpc: String,
    id: Value,

    /*
     * On a successful response this contains the method result.
     *
     * On an RPC error, result may be absent. Value::Null also lets future
     * methods such as getTransaction represent a legitimate null result.
     */
    #[serde(default)]
    result: Value,

    error: Option<JsonRpcErrorBody>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcErrorBody {
    code: i64,
    message: String,
    data: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RpcContext {
    slot: u64,
}

#[derive(Debug, Deserialize)]
struct RpcContextValue<T> {
    context: RpcContext,
    value: T,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcSignatureStatus {
    slot: u64,
    confirmation_status: ConfirmationStatus,
    err: Option<Value>,
}

/*
 * Raw getTransaction result returned by Solana when encoding=jsonParsed.
 */
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcConfirmedTransaction {
    slot: u64,
    block_time: Option<i64>,
    meta: Option<RpcTransactionMeta>,
    transaction: RpcParsedTransaction,
}

#[derive(Debug, Deserialize)]
struct RpcTransactionMeta {
    err: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct RpcParsedTransaction {
    signatures: Vec<String>,
    message: RpcParsedMessage,
}

#[derive(Debug, Deserialize)]
struct RpcParsedMessage {
    account_keys: Vec<RpcParsedAccountKey>,
    instructions: Vec<RpcTransactionInstruction>,
}

/*
 * jsonParsed account keys contain more fields such as writable and source.
 * We currently need only the address and signer flag; Serde ignores the
 * remaining response fields.
 */
#[derive(Debug, Deserialize)]
struct RpcParsedAccountKey {
    signer: bool,
    pubkey: String,
}

/*
 * jsonParsed may return either:
 *
 * 1. A fully parsed instruction for a program understood by the RPC node.
 * 2. A partially decoded instruction for a custom program such as ours.
 *
 * `untagged` asks Serde to select the variant based on which fields are
 * present instead of expecting an explicit enum tag in the JSON.
 */
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RpcTransactionInstruction {
    PartiallyDecoded(RpcPartiallyDecodedInstruction),
    Parsed(RpcParsedInstruction),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcPartiallyDecodedInstruction {
    program_id: String,
    accounts: Vec<String>,
    data: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RpcParsedInstruction {
    program_id: String,
    parsed: Value,
}

/*
 * Raw account representation returned by getMultipleAccounts.
 *
 * `data` is a two-element JSON array:
 *
 * [
 *     "<encoded account data>",
 *     "base64"
 * ]
 */
#[derive(Debug, Deserialize)]
struct RpcAccount {
    data: (String, String),
    executable: bool,
    lamports: u64,
    owner: String,
}

impl SolanaRpcClient {
    pub fn new(rpc_url: &str, timeout: Duration) -> Result<Self, SolanaRpcError> {
        let endpoint = reqwest::Url::parse(rpc_url)
            .map_err(|_| SolanaRpcError::InvalidRpcUrl(rpc_url.to_owned()))?;

        let http_client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(SolanaRpcError::ClientConstruction)?;

        Ok(Self {
            http_client,
            endpoint,
            next_request_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /*
     * Shared JSON-RPC transport:
     *
     * 1. Build a JSON-RPC 2.0 request.
     * 2. Send it through the configured HTTP endpoint.
     * 3. Distinguish HTTP errors from JSON-RPC errors.
     * 4. Verify that the response belongs to this request.
     * 5. Deserialize the method-specific result into T.
     */
    async fn call<T>(&self, method: &'static str, params: Value) -> Result<T, SolanaRpcError>
    where
        T: DeserializeOwned,
    {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);

        let request_body = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });

        let response = self
            .http_client
            .post(self.endpoint.clone())
            .json(&request_body)
            .send()
            .await
            .map_err(SolanaRpcError::Transport)?;

        let status = response.status();
        let response_bytes = response.bytes().await.map_err(SolanaRpcError::Transport)?;

        if !status.is_success() {
            let body = String::from_utf8_lossy(&response_bytes)
                .chars()
                .take(1_000)
                .collect();

            return Err(SolanaRpcError::HttpStatus {
                status: status.as_u16(),
                body,
            });
        }

        // deserialize the rpc response into the JsonRpcEnvelope struct
        let envelope: JsonRpcEnvelope =
            serde_json::from_slice(&response_bytes).map_err(SolanaRpcError::ResponseDecode)?;

        if envelope.jsonrpc != "2.0" {
            return Err(SolanaRpcError::InvalidResponse(format!(
                "expected JSON-RPC version 2.0, received {}",
                envelope.jsonrpc
            )));
        }

        if envelope.id.as_u64() != Some(request_id) {
            return Err(SolanaRpcError::ResponseIdMismatch);
        }

        if let Some(error) = envelope.error {
            return Err(SolanaRpcError::Rpc {
                code: error.code,
                message: error.message,
                data: error.data,
            });
        }

        let decoded_result: T =
            serde_json::from_value(envelope.result).map_err(SolanaRpcError::ResponseDecode)?;

        Ok(decoded_result)
    }

    /*
     * getGenesisHash proves that the endpoint is responding as a Solana
     * JSON-RPC server and tells us which cluster it belongs to.
     */
    pub async fn get_genesis_hash(&self) -> Result<String, SolanaRpcError> {
        let method = "getGenesisHash";
        let params = json!([]);

        self.call(method, params).await
    }

    /*
     * We use finalized block height when deciding expiration because our
     * authoritative projections also require finalized chain state.
     */
    pub async fn get_finalized_block_height(&self) -> Result<u64, SolanaRpcError> {
        let method = "getBlockHeight";
        let params = json!([
            {
                "commitment": "finalized"
            }
        ]);

        self.call(method, params).await
    }

    pub async fn get_signature_status(
        &self,
        signature: &str,
    ) -> Result<SignatureStatusLookup, SolanaRpcError> {
        let method = "getSignatureStatuses";
        let params = json!([
            [signature],
            {
                "searchTransactionHistory": true
            }
        ]);

        // why the response type? explain pls?
        let response: RpcContextValue<Vec<Option<RpcSignatureStatus>>> =
            self.call(method, params).await?;

        if response.value.len() != 1 {
            return Err(SolanaRpcError::InvalidResponse(format!(
                "expected one signature status, received {}",
                response.value.len()
            )));
        }

        let context_slot = response.context.slot;

        let rpc_status = response
            .value
            .into_iter()
            .next()
            .expect("length was checked above to be one");

        let found_status = rpc_status.map(|status| FoundSignatureStatus {
            transaction_slot: status.slot,
            confirmation_status: status.confirmation_status,
            error: status.err,
        });

        Ok(SignatureStatusLookup {
            context_slot,
            found_status,
        })
    }

    pub async fn get_finalized_transaction(
        &self,
        signature: &str,
    ) -> Result<Option<FinalizedTransaction>, SolanaRpcError> {
        let method = "getTransaction";

        let params = json!([
            signature,
            {
                "commitment": "finalized",
                "encoding": "jsonParsed",
                "maxSupportedTransactionVersion": 0
            }
        ]);

        let response: Option<RpcConfirmedTransaction> = self.call(method, params).await?;

        let response = match response {
            Some(response) => response,

            None => {
                return Ok(None);
            }
        };

        // meta might contain some error data
        let meta = response.meta.ok_or_else(|| {
            SolanaRpcError::InvalidResponse(
                "finalized transaction did not include transaction metadata".to_owned(),
            )
        })?;

        let RpcParsedTransaction {
            signatures,
            message,
        } = response.transaction;

        /*
         * A Solana transaction must contain at least one signature, and the first
         * signature must be the transaction ID requested from getTransaction.
         */
        let Some(transaction_id) = signatures.first() else {
            return Err(SolanaRpcError::InvalidResponse(
                "finalized transaction contained no signatures".to_owned(),
            ));
        };

        if transaction_id != signature {
            return Err(SolanaRpcError::InvalidResponse(format!(
                "getTransaction returned signature {transaction_id} when {signature} was requested"
            )));
        }

        let signer_addresses = message
            .account_keys
            .into_iter()
            .filter(|account| account.signer)
            .map(|account| account.pubkey)
            .collect();

        let mut instructions = Vec::with_capacity(message.instructions.len());

        for instruction in message.instructions {
            let normalized_instruction = match instruction {
                RpcTransactionInstruction::PartiallyDecoded(instruction) => {
                    let data = bs58::decode(instruction.data)
                        .into_vec()
                        .map_err(SolanaRpcError::InstructionDataDecode)?;

                    TransactionInstruction::PartiallyDecoded {
                        program_id: instruction.program_id,
                        accounts: instruction.accounts,
                        data,
                    }
                }

                RpcTransactionInstruction::Parsed(instruction) => TransactionInstruction::Parsed {
                    program_id: instruction.program_id,
                    parsed: instruction.parsed,
                },
            };

            instructions.push(normalized_instruction);
        }

        Ok(Some(FinalizedTransaction {
            slot: response.slot,
            block_time: response.block_time,
            error: meta.err,
            signatures,
            signer_addresses,
            instructions,
        }))
    }

    pub async fn get_finalized_accounts(
        &self,
        addresses: &[String],
        min_context_slot: u64,
    ) -> Result<FinalizedAccountsLookup, SolanaRpcError> {
        if addresses.is_empty() {
            return Err(SolanaRpcError::InvalidResponse(
                "getMultipleAccounts requires at least one address".to_owned(),
            ));
        }

        let method = "getMultipleAccounts";

        let params = json!([
            addresses,
            {
                "commitment": "finalized",
                "encoding": "base64",
                "minContextSlot": min_context_slot
            }
        ]);

        /*
         * The outer contextual response applies to the complete account
         * collection. Every account value was observed from this RPC
         * context.
         *
         * Each vector position is optional because an address may not
         * currently contain an account.
         */
        let response: RpcContextValue<Vec<Option<RpcAccount>>> = self.call(method, params).await?;

        let context_slot = response.context.slot;

        /*
         * We asked the RPC node not to serve state older than the
         * initialization transaction.
         *
         * Verify the response respected that requirement rather than
         * trusting it silently.
         */
        if context_slot < min_context_slot {
            return Err(SolanaRpcError::InvalidResponse(format!(
                "account context slot {context_slot} was older than minimum context slot {min_context_slot}"
            )));
        }

        /*
         * getMultipleAccounts promises positional correspondence:
         *
         * addresses[0] <-> response.value[0]
         * addresses[1] <-> response.value[1]
         *
         * A length mismatch would make that correspondence unsafe.
         */
        if response.value.len() != addresses.len() {
            return Err(SolanaRpcError::InvalidResponse(format!(
                "requested {} accounts but RPC returned {} account values",
                addresses.len(),
                response.value.len()
            )));
        }

        let mut accounts = Vec::with_capacity(addresses.len());

        /*
         * zip pairs each requested address with the account value returned
         * at the same position.
         */
        for (address, rpc_account) in addresses.iter().zip(response.value.into_iter()) {
            let account = match rpc_account {
                None => None,

                Some(rpc_account) => {
                    let (encoded_data, encoding) = rpc_account.data;

                    /*
                     * We specifically requested Base64. Reject another
                     * encoding instead of attempting to decode it using the
                     * wrong algorithm.
                     */
                    if encoding != "base64" {
                        return Err(SolanaRpcError::InvalidResponse(format!(
                            "expected Base64 account data for {address}, received {encoding}"
                        )));
                    }

                    let data = BASE64_STANDARD
                        .decode(encoded_data)
                        .map_err(SolanaRpcError::AccountDataDecode)?;

                    Some(FinalizedAccountSnapshot {
                        address: address.clone(),
                        owner: rpc_account.owner,
                        lamports: rpc_account.lamports,
                        executable: rpc_account.executable,
                        data,
                    })
                }
            };

            accounts.push(account);
        }

        Ok(FinalizedAccountsLookup {
            context_slot,
            accounts,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::{Json, Router, extract::State, http::StatusCode, routing::post};
    use serde_json::{Value, json};
    use tokio::net::TcpListener;

    use super::{
        ConfirmationStatus, FinalizedAccountSnapshot, FinalizedAccountsLookup,
        FoundSignatureStatus, SignatureStatusLookup, SolanaRpcClient, SolanaRpcError,
    };

    /*
     * Configuration for one local mock RPC server.
     *
     * Besides returning a configured response, the server verifies that the
     * client sent the expected RPC method and parameters.
     */
    #[derive(Clone)]
    struct MockRpcState {
        expected_method: &'static str,
        expected_params: Value,

        http_status: StatusCode,
        response_body: Value,

        /*
         * Normal JSON-RPC responses repeat the request ID. Some tests disable
         * this so that we can deliberately return an incorrect ID.
         */
        echo_request_id: bool,
    }

    async fn mock_rpc_handler(
        State(state): State<MockRpcState>,
        Json(request): Json<Value>,
    ) -> (StatusCode, Json<Value>) {
        let method_matches =
            request.get("method").and_then(Value::as_str) == Some(state.expected_method);

        let params_match = request.get("params") == Some(&state.expected_params);

        if !method_matches || !params_match {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "unexpected RPC request",
                    "expectedMethod": state.expected_method,
                    "expectedParams": state.expected_params,
                    "received": request,
                })),
            );
        }

        let mut response_body = state.response_body;

        if state.echo_request_id {
            let request_id = request.get("id").cloned().unwrap_or(Value::Null);

            if let Some(response_object) = response_body.as_object_mut() {
                response_object.insert("id".to_owned(), request_id);
            }
        }

        (state.http_status, Json(response_body))
    }

    /*
     * Bind to port zero so that the operating system selects an available
     * local port. This lets tests run without hard-coded port conflicts.
     */
    async fn spawn_mock_rpc(state: MockRpcState) -> String {
        let app = Router::new()
            .route("/", post(mock_rpc_handler))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("mock RPC listener should bind");

        let address = listener
            .local_addr()
            .expect("mock RPC listener should have an address");

        /*
         * Dropping the JoinHandle detaches the task. The Tokio test runtime
         * stops the server when the test completes.
         */
        let _server = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("mock RPC server should run");
        });

        format!("http://{address}")
    }

    fn test_client(endpoint: &str) -> SolanaRpcClient {
        SolanaRpcClient::new(endpoint, Duration::from_secs(2))
            .expect("test RPC client should be constructed")
    }

    #[tokio::test]
    async fn genesis_hash_is_decoded() {
        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getGenesisHash",
            expected_params: json!([]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": "test-genesis-hash",
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let genesis_hash = client
            .get_genesis_hash()
            .await
            .expect("genesis hash request should succeed");

        assert_eq!(genesis_hash, "test-genesis-hash");
    }

    #[tokio::test]
    async fn finalized_block_height_is_decoded() {
        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getBlockHeight",
            expected_params: json!([
                {
                    "commitment": "finalized"
                }
            ]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": 987_654_u64,
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let block_height = client
            .get_finalized_block_height()
            .await
            .expect("block height request should succeed");

        assert_eq!(block_height, 987_654);
    }

    #[tokio::test]
    async fn finalized_signature_without_error_is_decoded() {
        let signature = "finalized-success-signature";

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getSignatureStatuses",
            expected_params: json!([
                [signature],
                {
                    "searchTransactionHistory": true
                }
            ]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 500_000_u64
                    },
                    "value": [
                        {
                            "slot": 499_900_u64,
                            "confirmations": null,
                            "err": null,
                            "status": {
                                "Ok": null
                            },
                            "confirmationStatus": "finalized"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let lookup = client
            .get_signature_status(signature)
            .await
            .expect("signature lookup should succeed");

        assert_eq!(
            lookup,
            SignatureStatusLookup {
                context_slot: 500_000,
                found_status: Some(FoundSignatureStatus {
                    transaction_slot: 499_900,
                    confirmation_status: ConfirmationStatus::Finalized,
                    error: None,
                }),
            }
        );
    }

    #[tokio::test]
    async fn confirmed_signature_is_decoded() {
        let signature = "confirmed-signature";

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getSignatureStatuses",
            expected_params: json!([
                [signature],
                {
                    "searchTransactionHistory": true
                }
            ]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 700_u64
                    },
                    "value": [
                        {
                            "slot": 690_u64,
                            "err": null,
                            "confirmationStatus": "confirmed"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let lookup = client
            .get_signature_status(signature)
            .await
            .expect("signature lookup should succeed");

        assert_eq!(lookup.context_slot, 700);

        assert_eq!(
            lookup.found_status,
            Some(FoundSignatureStatus {
                transaction_slot: 690,
                confirmation_status: ConfirmationStatus::Confirmed,
                error: None,
            })
        );
    }

    #[tokio::test]
    async fn missing_signature_is_returned_as_none() {
        let signature = "missing-signature";

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getSignatureStatuses",
            expected_params: json!([
                [signature],
                {
                    "searchTransactionHistory": true
                }
            ]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 800_u64
                    },
                    "value": [
                        null
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let lookup = client
            .get_signature_status(signature)
            .await
            .expect("missing signature is still a successful RPC response");

        assert_eq!(lookup.context_slot, 800);
        assert_eq!(lookup.found_status, None);
    }

    #[tokio::test]
    async fn structured_transaction_error_is_preserved() {
        let signature = "failed-signature";

        let transaction_error = json!({
            "InstructionError": [
                0,
                {
                    "Custom": 6001
                }
            ]
        });

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getSignatureStatuses",
            expected_params: json!([
                [signature],
                {
                    "searchTransactionHistory": true
                }
            ]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 900_u64
                    },
                    "value": [
                        {
                            "slot": 890_u64,
                            "err": transaction_error.clone(),
                            "confirmationStatus": "finalized"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let lookup = client
            .get_signature_status(signature)
            .await
            .expect("failed transaction should still be decoded");

        let found_status = lookup
            .found_status
            .expect("signature should have been found");

        assert_eq!(
            found_status.confirmation_status,
            ConfirmationStatus::Finalized
        );

        assert_eq!(found_status.error, Some(transaction_error));
    }

    #[tokio::test]
    async fn http_rate_limit_is_reported_separately() {
        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getGenesisHash",
            expected_params: json!([]),

            http_status: StatusCode::TOO_MANY_REQUESTS,
            response_body: json!({
                "message": "rate limit exceeded"
            }),

            echo_request_id: false,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_genesis_hash()
            .await
            .expect_err("HTTP 429 should fail");

        match error {
            SolanaRpcError::HttpStatus { status, body } => {
                assert_eq!(status, 429);
                assert!(body.contains("rate limit exceeded"));
            }

            other => panic!("expected HTTP status error, received {other:?}"),
        }
    }

    #[tokio::test]
    async fn json_rpc_error_is_reported_separately() {
        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getGenesisHash",
            expected_params: json!([]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32602,
                    "message": "Invalid params",
                    "data": {
                        "field": "params"
                    }
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_genesis_hash()
            .await
            .expect_err("JSON-RPC error should fail");

        match error {
            SolanaRpcError::Rpc {
                code,
                message,
                data,
            } => {
                assert_eq!(code, -32602);
                assert_eq!(message, "Invalid params");
                assert_eq!(data, Some(json!({ "field": "params" })));
            }

            other => panic!("expected JSON-RPC error, received {other:?}"),
        }
    }

    #[tokio::test]
    async fn mismatched_response_id_is_rejected() {
        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getGenesisHash",
            expected_params: json!([]),

            http_status: StatusCode::OK,
            response_body: json!({
                "jsonrpc": "2.0",
                "id": 999,
                "result": "test-genesis-hash"
            }),

            echo_request_id: false,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_genesis_hash()
            .await
            .expect_err("mismatched response ID should fail");

        assert!(matches!(error, SolanaRpcError::ResponseIdMismatch));
    }

    #[tokio::test]
    async fn finalized_accounts_are_decoded_from_one_context() {
        let addresses = vec![
            "organization-pda".to_owned(),
            "owner-member-pda".to_owned(),
            "owner-member-wallet-pda".to_owned(),
        ];

        let minimum_context_slot = 1_000_u64;

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getMultipleAccounts",

            expected_params: json!([
                addresses.clone(),
                {
                    "commitment": "finalized",
                    "encoding": "base64",
                    "minContextSlot": minimum_context_slot
                }
            ]),

            http_status: StatusCode::OK,

            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 1_005_u64
                    },
                    "value": [
                        {
                            "data": ["AQID", "base64"],
                            "executable": false,
                            "lamports": 1_000_000_u64,
                            "owner": "expected-program-id",
                            "rentEpoch": 100_u64,
                            "space": 64
                        },
                        {
                            "data": ["BAUG", "base64"],
                            "executable": false,
                            "lamports": 2_000_000_u64,
                            "owner": "expected-program-id",
                            "rentEpoch": 100_u64,
                            "space": 96
                        },
                        {
                            "data": ["BwgJ", "base64"],
                            "executable": false,
                            "lamports": 3_000_000_u64,
                            "owner": "expected-program-id",
                            "rentEpoch": 100_u64,
                            "space": 72
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let lookup = client
            .get_finalized_accounts(&addresses, minimum_context_slot)
            .await
            .expect("finalized account lookup should succeed");

        assert_eq!(
            lookup,
            FinalizedAccountsLookup {
                context_slot: 1_005,

                accounts: vec![
                    Some(FinalizedAccountSnapshot {
                        address: "organization-pda".to_owned(),
                        owner: "expected-program-id".to_owned(),
                        lamports: 1_000_000,
                        executable: false,
                        data: vec![1, 2, 3],
                    }),
                    Some(FinalizedAccountSnapshot {
                        address: "owner-member-pda".to_owned(),
                        owner: "expected-program-id".to_owned(),
                        lamports: 2_000_000,
                        executable: false,
                        data: vec![4, 5, 6],
                    }),
                    Some(FinalizedAccountSnapshot {
                        address: "owner-member-wallet-pda".to_owned(),
                        owner: "expected-program-id".to_owned(),
                        lamports: 3_000_000,
                        executable: false,
                        data: vec![7, 8, 9],
                    }),
                ],
            }
        );
    }

    #[tokio::test]
    async fn missing_account_is_preserved_at_its_request_position() {
        let addresses = vec![
            "organization-pda".to_owned(),
            "owner-member-pda".to_owned(),
            "owner-member-wallet-pda".to_owned(),
        ];

        let minimum_context_slot = 2_000_u64;

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getMultipleAccounts",

            expected_params: json!([
                addresses.clone(),
                {
                    "commitment": "finalized",
                    "encoding": "base64",
                    "minContextSlot": minimum_context_slot
                }
            ]),

            http_status: StatusCode::OK,

            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 2_010_u64
                    },
                    "value": [
                        {
                            "data": ["AQID", "base64"],
                            "executable": false,
                            "lamports": 1_u64,
                            "owner": "expected-program-id"
                        },
                        null,
                        {
                            "data": ["BwgJ", "base64"],
                            "executable": false,
                            "lamports": 3_u64,
                            "owner": "expected-program-id"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let lookup = client
            .get_finalized_accounts(&addresses, minimum_context_slot)
            .await
            .expect("a missing account is still a valid RPC response");

        assert_eq!(lookup.context_slot, 2_010);
        assert_eq!(lookup.accounts.len(), 3);

        assert_eq!(
            lookup.accounts[0]
                .as_ref()
                .expect("organization account should exist")
                .address,
            "organization-pda"
        );

        assert_eq!(lookup.accounts[1], None);

        assert_eq!(
            lookup.accounts[2]
                .as_ref()
                .expect("member-wallet account should exist")
                .address,
            "owner-member-wallet-pda"
        );
    }

    #[tokio::test]
    async fn account_context_older_than_minimum_is_rejected() {
        let addresses = vec!["organization-pda".to_owned()];
        let minimum_context_slot = 3_000_u64;

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getMultipleAccounts",

            expected_params: json!([
                addresses.clone(),
                {
                    "commitment": "finalized",
                    "encoding": "base64",
                    "minContextSlot": minimum_context_slot
                }
            ]),

            http_status: StatusCode::OK,

            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 2_999_u64
                    },
                    "value": [
                        {
                            "data": ["AQID", "base64"],
                            "executable": false,
                            "lamports": 1_u64,
                            "owner": "expected-program-id"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_finalized_accounts(&addresses, minimum_context_slot)
            .await
            .expect_err("stale account context must be rejected");

        match error {
            SolanaRpcError::InvalidResponse(message) => {
                assert!(message.contains("2_999") || message.contains("2999"));
                assert!(message.contains("3_000") || message.contains("3000"));
            }

            other => {
                panic!("expected invalid-response error, received {other:?}");
            }
        }
    }

    #[tokio::test]
    async fn account_response_count_mismatch_is_rejected() {
        let addresses = vec![
            "organization-pda".to_owned(),
            "owner-member-pda".to_owned(),
            "owner-member-wallet-pda".to_owned(),
        ];

        let minimum_context_slot = 4_000_u64;

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getMultipleAccounts",

            expected_params: json!([
                addresses.clone(),
                {
                    "commitment": "finalized",
                    "encoding": "base64",
                    "minContextSlot": minimum_context_slot
                }
            ]),

            http_status: StatusCode::OK,

            /*
             * We requested three accounts, but the mock RPC returns only
             * two values. Using zip without a preceding length check would
             * silently omit the third expected account.
             */
            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 4_010_u64
                    },
                    "value": [
                        null,
                        null
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_finalized_accounts(&addresses, minimum_context_slot)
            .await
            .expect_err("response count mismatch must be rejected");

        match error {
            SolanaRpcError::InvalidResponse(message) => {
                assert!(message.contains("requested 3"));
                assert!(message.contains("returned 2"));
            }

            other => {
                panic!("expected invalid-response error, received {other:?}");
            }
        }
    }

    #[tokio::test]
    async fn unexpected_account_encoding_is_rejected() {
        let addresses = vec!["organization-pda".to_owned()];
        let minimum_context_slot = 5_000_u64;

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getMultipleAccounts",

            expected_params: json!([
                addresses.clone(),
                {
                    "commitment": "finalized",
                    "encoding": "base64",
                    "minContextSlot": minimum_context_slot
                }
            ]),

            http_status: StatusCode::OK,

            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 5_010_u64
                    },
                    "value": [
                        {
                            "data": ["AQID", "base58"],
                            "executable": false,
                            "lamports": 1_u64,
                            "owner": "expected-program-id"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_finalized_accounts(&addresses, minimum_context_slot)
            .await
            .expect_err("unexpected account encoding must be rejected");

        match error {
            SolanaRpcError::InvalidResponse(message) => {
                assert!(message.contains("base58"));
            }

            other => {
                panic!("expected invalid-response error, received {other:?}");
            }
        }
    }

    #[tokio::test]
    async fn invalid_base64_account_data_is_rejected() {
        let addresses = vec!["organization-pda".to_owned()];
        let minimum_context_slot = 6_000_u64;

        let endpoint = spawn_mock_rpc(MockRpcState {
            expected_method: "getMultipleAccounts",

            expected_params: json!([
                addresses.clone(),
                {
                    "commitment": "finalized",
                    "encoding": "base64",
                    "minContextSlot": minimum_context_slot
                }
            ]),

            http_status: StatusCode::OK,

            response_body: json!({
                "jsonrpc": "2.0",
                "id": null,
                "result": {
                    "context": {
                        "slot": 6_010_u64
                    },
                    "value": [
                        {
                            "data": [
                                "%%%not-valid-base64%%%",
                                "base64"
                            ],
                            "executable": false,
                            "lamports": 1_u64,
                            "owner": "expected-program-id"
                        }
                    ]
                }
            }),

            echo_request_id: true,
        })
        .await;

        let client = test_client(&endpoint);

        let error = client
            .get_finalized_accounts(&addresses, minimum_context_slot)
            .await
            .expect_err("invalid Base64 data must be rejected");

        assert!(matches!(error, SolanaRpcError::AccountDataDecode(_)));
    }
}
