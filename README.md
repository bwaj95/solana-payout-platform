# Solana Payout Platform

An organization-governed payout protocol for securely paying employees, contractors, contributors, vendors, service providers, and other individual or organizational recipients on Solana.

The current build includes a deployed Anchor public SPL-token settlement rail, a TypeScript devnet client, and a Rust/Axum/PostgreSQL control-plane foundation. The backend implements wallet authentication, organization provisioning, idempotent transaction tracking, a leased PostgreSQL job queue, finalized Solana transaction validation, on-chain account validation, and atomic reconciliation settlement.

The public payout rail is working end-to-end on Solana devnet. The reconciliation worker runtime, broader product APIs, web dashboard, and MagicBlock private settlement rail remain under active development.

## Devnet deployment

| Item                   | Value                                                                                                                                                   |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Network                | Solana Devnet                                                                                                                                           |
| Program ID             | `5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv`                                                                                                          |
| Program Explorer       | [View deployed program](https://explorer.solana.com/address/5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv?cluster=devnet)                                |
| Deployment transaction | [rTaA7y...2k8ts](https://explorer.solana.com/tx/rTaA7yUGYv8fwZUV5JWD3joeGpCcDLFY1TAfHWqR4u4y7JdnujY9XdHss3gAiDaNWbeB7REis3kHF1FfZd2k8ts?cluster=devnet) |
| End-to-end execution   | [37i5gB...JtdU](https://explorer.solana.com/tx/37i5gBNoWZDRL812VqFkjhcLz4f4qSW4vSHxuo9NctTNS3kaK5psm2L9cSEVoHQWeENcpSs3C3ZU5R4CePugJtdU?cluster=devnet) |
| Verified payment PDA   | `7vhh2CoWvCT2zhsR6AsyUAgh1FFHueAGLiGsfLRjJUQe`                                                                                                          |

The devnet end-to-end script created a fresh organization, two approvers, an executor, a demo SPL mint and vault, a recipient, a two-of-two policy, and a payment. The second approval reserved the funds atomically; execution transferred 2.5 demo tokens and verified the final `Paid` and `Consumed` states with zero remaining reservation.

![Successful devnet end-to-end payout](docs/devnet-e2e-success.png)

## Test evidence

The deterministic Rust integration suite contains **132 passing tests** across 12 instruction-focused test files. It runs against LiteSVM and covers happy paths, authorization failures, cross-organization substitution, malformed policy and approval witnesses, state-transition safety, reservation accounting, wallet rotation, cancellation, pausing, execution, and vault withdrawals.

The live devnet scripts separately prove that the deployed binary can be invoked and that the complete governed payout flow succeeds on the public network.

![All 132 Rust integration tests passing](docs/all-tests-passing.png)

## Current implementation status

| Component | Status |
| --- | --- |
| Anchor public SPL payout rail | Deployed and working on devnet |
| Two-of-two governed payment flow | Demonstrated end-to-end |
| Anchor integration suite | 132 tests passing |
| TypeScript devnet client | Working |
| Wallet authentication and sessions | Implemented |
| Organization and owner provisioning | Implemented |
| Transaction submission tracking | Implemented with idempotency protection |
| PostgreSQL job queue | Implemented with leases, fencing, retries, deferral, recovery, and dead-lettering |
| Finalized Solana reconciliation processor | Implemented and tested |
| Standalone reconciliation worker runtime | Next implementation step |
| Product APIs and web dashboard | Planned next phase |
| MagicBlock private payout rail | Experimental integration in progress |

## Problem and solution

Business payouts require more than a token transfer. A company needs to know who prepared a payment, which policy governed it, who approved it, whether enough funds remain available, which recipient wallet was authorized, and whether a payment can be safely cancelled or executed.

This program models those controls directly on-chain:

- organization-scoped membership and role-based permissions;
- immutable, versioned approval policies;
- approvals bound to exact payment terms and authorization revisions;
- vault reservation accounting before execution;
- recipient wallet rotation with revision invalidation;
- scheduled execution, cancellation, emergency pause, and safe withdrawal;
- events for indexing and audit trails.

## Architecture

![Solana Payout Platform architecture](docs/architecture-overview.svg)

The TypeScript client derives every required PDA and submits explicitly named accounts. Anchor validates declared accounts, while the handlers independently validate dynamic policy and approval witness accounts. The vault PDA owns its canonical token account and signs the final `TransferChecked` CPI with program-derived signer seeds.

### Payment lifecycle

Each state box below shows the two independent state fields stored in the Payment account: `PaymentState` describes the payment's business lifecycle, while `ReservationState` describes what happened to its reserved vault funds.

![Payment and reservation state lifecycle](docs/payment-lifecycle.svg)

Approval and reservation happen atomically when the threshold becomes visible. If the vault lacks available funds, approvals remain recorded and the payment moves to `AwaitingFunds`; an authorized finalizer can retry reservation after funding. Execution consumes the reservation and payment state in the same transaction as the SPL transfer.

## PDA account model

![Solana Payout Platform Accounts](docs/Solana-Payout-Platform-Accounts.png)

All business state is namespaced by the organization PDA.

| Account               | PDA seeds                                                                                 | Purpose                                                                   |
| --------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Organization          | `organization`, creator, organization ID                                                  | Root configuration, owner member, pause state                             |
| Member                | `member`, organization, member ID                                                         | Authorized wallet, authorization revision, roles, active state            |
| MemberWallet          | `member_wallet`, organization, authorized wallet                                          | Prevents one wallet from representing multiple members in an organization |
| VaultState            | `vault`, organization, vault ID                                                           | SPL asset, aggregate reservation, active state                            |
| Recipient             | `recipient`, organization, recipient ID                                                   | Current destination wallet and wallet revision                            |
| ApprovalPolicyVersion | `policy`, organization, policy ID, version                                                | Immutable eligible-member set and threshold                               |
| Payment               | `payment`, organization, payment ID                                                       | Frozen payout terms and payment/reservation states                        |
| Approval              | `approval`, organization, payment ID, payment revision, member ID, authorization revision | One member's approval of one exact payment revision                       |

The SPL vault account is the canonical associated token account whose authority is the `VaultState` PDA. Recipient token accounts must also be canonical ATAs for the configured mint and current destination wallet.

## Roles and authorization

Roles are stored as a `u16` bitmask, allowing a member to hold multiple responsibilities:

| Role     | Current responsibility                                                                                        |
| -------- | ------------------------------------------------------------------------------------------------------------- |
| Owner    | Organization ownership identity; assigned only during initialization                                          |
| Admin    | Member, policy, vault, recipient, payment, pause, cancellation, and withdrawal administration where permitted |
| Preparer | Recipient registration and payment creation                                                                   |
| Approver | Payment approval when also listed in the exact policy version                                                 |
| Executor | Finalization and execution of approved payments                                                               |
| Treasury | Withdrawal of unreserved vault funds                                                                          |
| Finance  | Reserved in the current model for later finance workflows                                                     |

Permission is never based on a role bit alone. The supplied Member must be the canonical organization member PDA, be active, contain the required role, and store the transaction signer's wallet. Policy membership is checked separately for approvers.

## Security properties

- **Canonical identity:** PDA seeds, stored bumps, ownership, and organization relationships prevent account substitution.
- **Wallet uniqueness:** `MemberWallet` acts as an organization-scoped wallet index and is created atomically with the Member account.
- **Exact-term approvals:** a domain-separated SHA-256 hash binds organization, payment and recipient revisions, destination, vault, mint, amount, policy, settlement rail, and execution time.
- **Revision invalidation:** payment revisions, member authorization revisions, and recipient wallet revisions prevent stale authority or destination data from being reused.
- **Untrusted witness validation:** every Approval and Member pair supplied through `remaining_accounts` is re-derived, deserialized, relationship-checked, deduplicated, and checked for current validity.
- **Reservation invariant:** approved obligations increase `reserved_total`; execution and cancellation decrease only the affected payment's amount using checked arithmetic.
- **Safe withdrawals:** withdrawals are limited to `vault_balance - reserved_total`, preserving funds committed to approved payments.
- **Atomic settlement:** token transfer, reservation consumption, and payment-state transition succeed or roll back together.
- **Explicit stopping controls:** organization pause blocks risk-increasing operations and execution, while cancellation remains available to release a held reservation.
- **Immutable approval policy:** each payment references one exact policy-version PDA rather than mutable organization-wide policy data.

## Program instructions

| Instruction                 | Authorized actor  | Result                                                                 |
| --------------------------- | ----------------- | ---------------------------------------------------------------------- |
| `initialize_organization`   | Creator           | Atomically creates the organization, owner Member, and wallet index    |
| `create_member`             | Admin             | Creates a role-bearing Member and unique wallet index                  |
| `create_policy_version`     | Admin             | Creates an immutable threshold policy from validated approver Members  |
| `initialize_vault`          | Admin             | Creates an SPL VaultState and its canonical vault ATA                  |
| `register_recipient`        | Admin or Preparer | Registers a destination wallet verified against its canonical ATA      |
| `create_payment`            | Admin or Preparer | Freezes payout terms and creates a pending payment                     |
| `approve_payment`           | Eligible Approver | Records an approval and attempts atomic threshold reservation          |
| `finalize_payment_approval` | Admin or Executor | Revalidates approval witnesses and retries reservation                 |
| `execute_spl_payment`       | Executor          | Transfers tokens and atomically consumes payment and reservation state |
| `cancel_payment`            | Admin             | Cancels a live payment and releases its reservation when held          |
| `rotate_recipient_wallet`   | Admin or Preparer | Replaces the recipient destination and increments its revision         |
| `set_organization_paused`   | Admin             | Activates or clears the organization's emergency stop                  |
| `withdraw_spl_vault_funds`  | Admin or Treasury | Withdraws only liquidity not committed to reservations                 |

## Test coverage

| Test file                          |   Tests | Main coverage                                               |
| ---------------------------------- | ------: | ----------------------------------------------------------- |
| `test_initialize_organization.rs`  |       3 | Atomic initialization, duplicates, rollback                 |
| `test_create_member.rs`            |       7 | Admin authorization, roles, wallet uniqueness               |
| `test_initialize_vault.rs`         |       7 | Vault creation, mint and organization isolation             |
| `test_register_recipient.rs`       |      10 | Identity validation, canonical ATA, authorization           |
| `test_rotate_recipient_wallet.rs`  |      13 | Rotation, recovery, revision invalidation                   |
| `test_create_policy_version.rs`    |       6 | Thresholds, eligible members, immutability                  |
| `test_create_payment.rs`           |      22 | Cross-account relationships, terms, roles, invalid state    |
| `test_approve_payment.rs`          |      19 | Witness validation, threshold, reservation, stale approvals |
| `test_execute_spl_payment.rs`      |      10 | Scheduling, role gates, transfer, replay protection         |
| `test_cancel_payment.rs`           |      12 | State matrix and precise reservation release                |
| `test_set_organization_paused.rs`  |       7 | Emergency pause authorization and behavior                  |
| `test_withdraw_spl_vault_funds.rs` |      16 | Unreserved-balance invariant and destination safety         |
| **Total**                          | **132** | **Happy paths and adversarial cases**                       |

## Run locally

### Tested toolchain

- Rust `1.98.0`
- Solana CLI `3.1.10`
- Anchor CLI `1.0.2`
- Node.js `24.20.0`
- pnpm `11.24.0`

### Install and verify

```bash
pnpm install
anchor build
cargo test --workspace
pnpm exec tsc --noEmit
```

The Rust tests use LiteSVM and do not require a local validator.

### Run the devnet checks

Configure a funded devnet wallet at `~/.config/solana/id.json`, then run:

```bash
solana config set --url devnet
solana balance

pnpm devnet:smoke
pnpm devnet:e2e
```

`devnet:smoke` verifies program executability and organization initialization. `devnet:e2e` creates isolated demo state and a new demo SPL mint, then executes and verifies the full payout lifecycle. It does not use real USDC or mainnet assets.

## Repository structure

```text
programs/solana-payout-platform/
├── src/instructions/       # Anchor validation and instruction handlers
├── src/state.rs            # PDA accounts and payment state machines
├── src/events.rs           # Audit and indexing events
└── tests/                  # 132 LiteSVM integration tests

backend/
├── src/auth.rs             # Wallet challenges, signature verification and sessions
├── src/jobs.rs             # PostgreSQL job queue, leases, fencing and recovery
├── src/solana_rpc.rs       # Typed Solana JSON-RPC client
├── src/transaction_validation.rs
├── src/account_validation.rs
├── src/reconciliation.rs   # Atomic reconciliation settlement
├── src/reconciliation_worker.rs
└── tests/                  # Queue, concurrency and reconciliation tests

clients/solana/             # IDL, program construction and PDA helpers
migrations/                 # Versioned PostgreSQL schema
infrastructure/             # Local PostgreSQL Docker Compose setup
scripts/                    # Authentication and live devnet flows
docs/                       # Architecture diagrams and test evidence
.sqlx/                      # Offline SQLx compile-time query metadata
```

## Current scope and next phase

The current public settlement rail is deployed and proves the governed payout lifecycle independently on Solana devnet. The backend foundation now covers authenticated organization provisioning, idempotent transaction submissions, reliable PostgreSQL job processing, typed Solana RPC access, finalized transaction and account validation, and atomic reconciliation settlement.

The next implementation phase is to:

1. wire the reconciliation processor into a continuously running worker process;
2. complete APIs for members, recipients, vaults, policies, payments, and approvals;
3. build the wallet-connected web dashboard;
4. deploy the API, PostgreSQL database, worker, and frontend;
5. integrate MagicBlock Private Ephemeral Rollups as a private SPL settlement rail;
6. reconcile private and public outcomes without exposing sensitive payout data.

The private-settlement research and its currently reproducible PER integration issue are documented in the [three-recipient privacy lab](https://github.com/bwaj95/three-recipient-privacy-lab).

## Demo

[Watch the governed devnet payout demonstration](https://www.youtube.com/watch?v=Fo1eSrDH8zQ)

The demonstration creates an organization, two approvers, an executor, an SPL vault, a recipient, and a two-of-two approval policy. It then prepares a payment, records both approvals, reserves the funds, executes the transfer, and verifies the final `Paid` and `Consumed` states.