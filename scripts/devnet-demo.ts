import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { stdin, stdout } from "node:process";
import { createInterface } from "node:readline/promises";
import { setTimeout as delay } from "node:timers/promises";

import { BN } from "@anchor-lang/core";
import {
    ASSOCIATED_TOKEN_PROGRAM_ID,
    createMint,
    getAccount,
    getAssociatedTokenAddressSync,
    getOrCreateAssociatedTokenAccount,
    mintTo,
    TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import {
    AccountMeta,
    Connection,
    Keypair,
    LAMPORTS_PER_SOL,
    PublicKey,
    sendAndConfirmTransaction,
    SystemProgram,
    Transaction,
    TransactionInstruction,
} from "@solana/web3.js";

import {
    INITIAL_AUTHORIZATION_REVISION,
    INITIAL_PAYMENT_REVISION,
    ROLE_APPROVER,
    ROLE_EXECUTOR,
} from "../clients/solana/constants";
import {
    findApprovalPda,
    findMemberPda,
    findMemberWalletPda,
    findOrganizationPda,
    findPaymentPda,
    findPolicyVersionPda,
    findRecipientPda,
    findVaultStatePda,
} from "../clients/solana/pdas";
import { createProgram, PROGRAM_ID } from "../clients/solana/program";

const DEVNET_GENESIS_HASH = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

const MINT_DECIMALS = 6;
const TOKEN_BASE_UNITS = 10n ** BigInt(MINT_DECIMALS);

// 10,000 demo tokens using USDC-style six-decimal precision.
const VAULT_FUNDING_AMOUNT = 10_000n * TOKEN_BASE_UNITS;

// Pay the recipient 1,000 demo tokens.
const PAYMENT_AMOUNT = 1_000n * TOKEN_BASE_UNITS;

// Approvers pay rent for their Approval PDAs.
// Executor pays the execution transaction fee.
const TEMPORARY_SIGNER_FUNDING_LAMPORTS = 10_000_000;

// A small delay reduces transaction bursts against the RPC provider.
const RPC_COOLDOWN_MS = 600;

const presenter = createInterface({
    input: stdin,
    output: stdout,
});

function loadDefaultWallet(): Keypair {
    const walletPath = join(homedir(), ".config", "solana", "id.json");

    const secretKey = JSON.parse(readFileSync(walletPath, "utf8")) as number[];

    return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}

function createUniqueU64(): BN {
    const suffix = Math.floor(Math.random() * 1_000)
        .toString()
        .padStart(3, "0");

    return new BN(`${Date.now()}${suffix}`);
}

function explorerUrl(signature: string): string {
    return `https://explorer.solana.com/tx/${signature}?cluster=devnet`;
}

function describeRpcUrl(rpcUrl: string): string {
    try {
        const parsedUrl = new URL(rpcUrl);

        // Do not print the Alchemy API key during the presentation.
        return `${parsedUrl.protocol}//${parsedUrl.host}/<redacted>`;
    } catch {
        return "<configured RPC endpoint>";
    }
}

function formatTokenAmount(amount: bigint): string {
    const whole = amount / TOKEN_BASE_UNITS;
    const remainder = amount % TOKEN_BASE_UNITS;

    if (remainder === 0n) {
        return whole.toString();
    }

    const fractionalPart = remainder
        .toString()
        .padStart(MINT_DECIMALS, "0")
        .replace(/0+$/, "");

    return `${whole}.${fractionalPart}`;
}

function printField(label: string, value: string | number | boolean): void {
    console.log(`${label.padEnd(30)}${value}`);
}

function readonlyAccount(pubkey: PublicKey): AccountMeta {
    return {
        pubkey,
        isWritable: false,
        isSigner: false,
    };
}

function hasVariant(value: object, variant: string): boolean {
    return variant in value;
}

async function rpcCooldown(): Promise<void> {
    await delay(RPC_COOLDOWN_MS);
}

async function beginStage(title: string, description: string): Promise<void> {
    console.log("");
    console.log("=".repeat(78));
    console.log(title);
    console.log(description);
    console.log("=".repeat(78));

    await presenter.question("Press Enter to execute this stage...");
    console.log("");
}

async function sendInstruction(
    connection: Connection,
    label: string,
    instruction: TransactionInstruction,
    signer: Keypair
): Promise<string> {
    const signature = await sendAndConfirmTransaction(
        connection,
        new Transaction().add(instruction),
        [signer],
        {
            commitment: "confirmed",
        }
    );

    console.log(`${label}:`);
    console.log(`  Signature: ${signature}`);
    console.log(`  Explorer:  ${explorerUrl(signature)}`);

    await rpcCooldown();

    return signature;
}

async function main(): Promise<void> {
    const rpcUrl = process.env.SOLANA_RPC_URL?.trim();

    if (!rpcUrl) {
        throw new Error(
            "SOLANA_RPC_URL is not configured. Export the Alchemy devnet RPC URL before running the demo."
        );
    }

    const connection = new Connection(rpcUrl, "confirmed");
    const program = createProgram(connection);
    const creator = loadDefaultWallet();

    console.log("");
    console.log("SOLANA PAYOUT PLATFORM");
    console.log("Interactive Devnet Payment Demonstration");
    console.log("");

    printField("RPC provider", describeRpcUrl(rpcUrl));
    printField("Program ID", PROGRAM_ID.toBase58());
    printField("Creator wallet", creator.publicKey.toBase58());

    const genesisHash = await connection.getGenesisHash();

    if (genesisHash !== DEVNET_GENESIS_HASH) {
        throw new Error(
            `The configured RPC is not Solana Devnet. Received genesis hash: ${genesisHash}`
        );
    }

    printField("Network", "Solana Devnet");

    const deployedProgram = await connection.getAccountInfo(
        PROGRAM_ID,
        "confirmed"
    );

    if (!deployedProgram?.executable) {
        throw new Error(
            "The payout program is not executable on the configured devnet RPC."
        );
    }

    printField("Program executable", true);

    const creatorBalance = await connection.getBalance(
        creator.publicKey,
        "confirmed"
    );

    printField("Creator SOL balance", `${creatorBalance / LAMPORTS_PER_SOL} SOL`);

    if (creatorBalance < 100_000_000) {
        throw new Error("The demo wallet should have at least 0.1 devnet SOL.");
    }

    console.log("");
    console.log(
        "All tokens and accounts created by this script are disposable devnet demo assets."
    );

    /*
     * These temporary wallets remain only in this running process.
     * They are not written to disk.
     */
    const approverOne = Keypair.generate();
    const approverTwo = Keypair.generate();
    const executor = Keypair.generate();
    const recipientWallet = Keypair.generate();

    const organizationId = createUniqueU64();

    const ownerMemberId = new BN(1);
    const approverOneMemberId = new BN(2);
    const approverTwoMemberId = new BN(3);
    const executorMemberId = new BN(4);

    const vaultId = new BN(1);
    const recipientId = new BN(1);
    const policyId = new BN(1);
    const policyVersionNumber = new BN(1);
    const paymentId = new BN(1);

    const organization = findOrganizationPda(creator.publicKey, organizationId);

    const ownerMember = findMemberPda(organization, ownerMemberId);

    const ownerMemberWallet = findMemberWalletPda(
        organization,
        creator.publicKey
    );

    const approverOneMember = findMemberPda(organization, approverOneMemberId);

    const approverTwoMember = findMemberPda(organization, approverTwoMemberId);

    const executorMember = findMemberPda(organization, executorMemberId);

    const approverOneMemberWallet = findMemberWalletPda(
        organization,
        approverOne.publicKey
    );

    const approverTwoMemberWallet = findMemberWalletPda(
        organization,
        approverTwo.publicKey
    );

    const executorMemberWallet = findMemberWalletPda(
        organization,
        executor.publicKey
    );

    const vaultState = findVaultStatePda(organization, vaultId);

    const recipient = findRecipientPda(organization, recipientId);

    const policyVersion = findPolicyVersionPda(
        organization,
        policyId,
        policyVersionNumber
    );

    const payment = findPaymentPda(organization, paymentId);

    /*
     * STAGE 1
     * Create the organization and its role-separated members.
     */
    await beginStage(
        "STAGE 1: ORGANIZATION AND ROLE SETUP",
        "Create the organization, owner, two approvers, and one executor."
    );

    const initializeOrganizationInstruction = await program.methods
        .initializeOrganization(organizationId, ownerMemberId)
        .accountsStrict({
            creator: creator.publicKey,
            organization,
            ownerMember,
            ownerMemberWallet,
            systemProgram: SystemProgram.programId,
        })
        .instruction();

    await sendInstruction(
        connection,
        "[1.1] Initialize organization and owner",
        initializeOrganizationInstruction,
        creator
    );

    const fundTemporarySigners = new Transaction();

    for (const recipientPublicKey of [
        approverOne.publicKey,
        approverTwo.publicKey,
        executor.publicKey,
    ]) {
        fundTemporarySigners.add(
            SystemProgram.transfer({
                fromPubkey: creator.publicKey,
                toPubkey: recipientPublicKey,
                lamports: TEMPORARY_SIGNER_FUNDING_LAMPORTS,
            })
        );
    }

    const fundingSignature = await sendAndConfirmTransaction(
        connection,
        fundTemporarySigners,
        [creator],
        {
            commitment: "confirmed",
        }
    );

    console.log("[1.2] Fund temporary approver and executor wallets:");
    console.log(`  Signature: ${fundingSignature}`);
    console.log(`  Explorer:  ${explorerUrl(fundingSignature)}`);

    await rpcCooldown();

    const members = [
        {
            label: "Approver one",
            id: approverOneMemberId,
            wallet: approverOne.publicKey,
            roles: ROLE_APPROVER,
            member: approverOneMember,
            memberWallet: approverOneMemberWallet,
        },
        {
            label: "Approver two",
            id: approverTwoMemberId,
            wallet: approverTwo.publicKey,
            roles: ROLE_APPROVER,
            member: approverTwoMember,
            memberWallet: approverTwoMemberWallet,
        },
        {
            label: "Executor",
            id: executorMemberId,
            wallet: executor.publicKey,
            roles: ROLE_EXECUTOR,
            member: executorMember,
            memberWallet: executorMemberWallet,
        },
    ];

    for (const memberDefinition of members) {
        const createMemberInstruction = await program.methods
            .createMember(
                memberDefinition.id,
                memberDefinition.wallet,
                memberDefinition.roles
            )
            .accountsStrict({
                authority: creator.publicKey,
                organization,
                adminMember: ownerMember,
                member: memberDefinition.member,
                memberWallet: memberDefinition.memberWallet,
                systemProgram: SystemProgram.programId,
            })
            .instruction();

        await sendInstruction(
            connection,
            `[1.3] Create ${memberDefinition.label}`,
            createMemberInstruction,
            creator
        );
    }

    const storedOrganization = await program.account.organization.fetch(
        organization
    );

    const storedOwnerMember = await program.account.member.fetch(ownerMember);

    console.log("");
    console.log("ORGANIZATION SETUP COMPLETE");
    console.log("");

    printField("Organization ID", organizationId.toString());
    printField("Organization PDA", organization.toBase58());
    printField("Organization paused", storedOrganization.paused);
    printField("Owner Member PDA", ownerMember.toBase58());
    printField("Owner roles bitmask", storedOwnerMember.roles);
    printField("Approver One Member", approverOneMember.toBase58());
    printField("Approver One role", "APPROVER");
    printField("Approver Two Member", approverTwoMember.toBase58());
    printField("Approver Two role", "APPROVER");
    printField("Executor Member", executorMember.toBase58());
    printField("Executor role", "EXECUTOR");

    /*
     * STAGE 2
     * Create and fund an SPL vault, then register the recipient.
     */
    await beginStage(
        "STAGE 2: VAULT AND RECIPIENT",
        "Create a six-decimal demo token, fund the vault with 10,000 tokens, and register the recipient."
    );

    const mint = await createMint(
        connection,
        creator,
        creator.publicKey,
        null,
        MINT_DECIMALS,
        undefined,
        {
            commitment: "confirmed",
        }
    );

    console.log("[2.1] Create demo USDC-style mint:");
    console.log(`  Mint:     ${mint.toBase58()}`);
    console.log(`  Decimals: ${MINT_DECIMALS}`);

    await rpcCooldown();

    const vaultAta = getAssociatedTokenAddressSync(mint, vaultState, true);

    const initializeVaultInstruction = await program.methods
        .initializeVault(vaultId)
        .accountsStrict({
            authority: creator.publicKey,
            organization,
            adminMember: ownerMember,
            vaultState,
            mint,
            vaultTokenAccount: vaultAta,
            tokenProgram: TOKEN_PROGRAM_ID,
            associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
            systemProgram: SystemProgram.programId,
        })
        .instruction();

    await sendInstruction(
        connection,
        "[2.2] Initialize SPL vault",
        initializeVaultInstruction,
        creator
    );

    const mintToSignature = await mintTo(
        connection,
        creator,
        mint,
        vaultAta,
        creator,
        VAULT_FUNDING_AMOUNT,
        [],
        {
            commitment: "confirmed",
        }
    );

    console.log("[2.3] Fund vault:");
    console.log(
        `  Amount:    ${formatTokenAmount(VAULT_FUNDING_AMOUNT)} demo tokens`
    );
    console.log(`  Signature: ${mintToSignature}`);
    console.log(`  Explorer:  ${explorerUrl(mintToSignature)}`);

    await rpcCooldown();

    const destinationAta = await getOrCreateAssociatedTokenAccount(
        connection,
        creator,
        mint,
        recipientWallet.publicKey,
        false,
        "confirmed"
    );

    console.log("[2.4] Create recipient token account:");
    console.log(`  Recipient wallet: ${recipientWallet.publicKey.toBase58()}`);
    console.log(`  Recipient ATA:    ${destinationAta.address.toBase58()}`);

    await rpcCooldown();

    const registerRecipientInstruction = await program.methods
        .registerRecipient(recipientId, recipientWallet.publicKey)
        .accountsStrict({
            authority: creator.publicKey,
            organization,
            registrarMember: ownerMember,
            vaultState,
            recipient,
            mint,
            destinationTokenAccount: destinationAta.address,
            systemProgram: SystemProgram.programId,
        })
        .instruction();

    await sendInstruction(
        connection,
        "[2.5] Register recipient",
        registerRecipientInstruction,
        creator
    );

    const storedVault = await program.account.vaultState.fetch(vaultState);

    const storedRecipient = await program.account.recipient.fetch(recipient);

    const fundedVaultTokenAccount = await getAccount(connection, vaultAta);

    const initialDestinationTokenAccount = await getAccount(
        connection,
        destinationAta.address
    );

    console.log("");
    console.log("VAULT AND RECIPIENT READY");
    console.log("");

    printField("Mint", mint.toBase58());
    printField("Mint decimals", MINT_DECIMALS);
    printField("Vault State PDA", vaultState.toBase58());
    printField("Vault token account", vaultAta.toBase58());
    printField(
        "Vault token balance",
        `${formatTokenAmount(fundedVaultTokenAccount.amount)} demo tokens`
    );
    printField(
        "Vault reserved total",
        `${formatTokenAmount(
            BigInt(storedVault.reservedTotal.toString())
        )} demo tokens`
    );
    printField("Recipient PDA", recipient.toBase58());
    printField(
        "Recipient destination",
        storedRecipient.currentDestination.toBase58()
    );
    printField(
        "Recipient token balance",
        `${formatTokenAmount(initialDestinationTokenAccount.amount)} demo tokens`
    );

    /*
     * STAGE 3
     * Create an immutable 2-of-2 approval policy.
     */
    await beginStage(
        "STAGE 3: TWO-OF-TWO APPROVAL POLICY",
        "Create an immutable policy requiring both registered approvers."
    );

    const createPolicyInstruction = await program.methods
        .createPolicyVersion(policyId, policyVersionNumber, 2)
        .accountsStrict({
            authority: creator.publicKey,
            organization,
            adminMember: ownerMember,
            approvalPolicyVersion: policyVersion,
            systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
            readonlyAccount(approverOneMember),
            readonlyAccount(approverTwoMember),
        ])
        .instruction();

    await sendInstruction(
        connection,
        "[3.1] Create two-of-two approval policy",
        createPolicyInstruction,
        creator
    );

    const storedPolicy = await program.account.approvalPolicyVersion.fetch(
        policyVersion
    );

    console.log("");
    console.log("APPROVAL POLICY CREATED");
    console.log("");

    printField("Policy PDA", policyVersion.toBase58());
    printField("Policy ID", storedPolicy.policyId.toString());
    printField("Policy version", storedPolicy.version.toString());
    printField("Threshold", `${storedPolicy.threshold} of 2`);
    printField("Eligible member count", storedPolicy.eligibleMembers.length);
    printField("Policy enabled", storedPolicy.enabled);
    printField("Eligible Approver One", approverOneMember.toBase58());
    printField("Eligible Approver Two", approverTwoMember.toBase58());

    /*
     * STAGE 4
     * Create a payment governed by that exact policy version.
     */
    await beginStage(
        "STAGE 4: CREATE PAYMENT",
        "Create a 1,000-token payment governed by the exact two-of-two policy version."
    );

    const createPaymentInstruction = await program.methods
        .createPayment(paymentId, new BN(PAYMENT_AMOUNT.toString()), new BN(0))
        .accountsStrict({
            authority: creator.publicKey,
            organization,
            preparerMember: ownerMember,
            recipient,
            vaultState,
            approvalPolicyVersion: policyVersion,
            payment,
            systemProgram: SystemProgram.programId,
        })
        .instruction();

    await sendInstruction(
        connection,
        "[4.1] Create payment",
        createPaymentInstruction,
        creator
    );

    const createdPayment = await program.account.payment.fetch(payment);

    const vaultBeforeApprovals = await program.account.vaultState.fetch(
        vaultState
    );

    if (!hasVariant(createdPayment.paymentState, "pendingApproval")) {
        throw new Error("New payment did not enter PendingApproval state.");
    }

    if (!hasVariant(createdPayment.reservationState, "none")) {
        throw new Error("New payment did not begin with reservation state None.");
    }

    if (!vaultBeforeApprovals.reservedTotal.isZero()) {
        throw new Error("Vault should not reserve funds during payment creation.");
    }

    console.log("");
    console.log("PAYMENT CREATED");
    console.log("");

    printField("Payment PDA", payment.toBase58());
    printField("Payment ID", createdPayment.paymentId.toString());
    printField(
        "Amount",
        `${formatTokenAmount(BigInt(createdPayment.amount.toString()))} demo tokens`
    );
    printField("Recipient PDA", createdPayment.recipient.toBase58());
    printField("Vault State PDA", createdPayment.vault.toBase58());
    printField("Policy Version PDA", createdPayment.policyVersion.toBase58());
    printField("Payment state", "PendingApproval");
    printField("Reservation state", "None");
    printField("Vault reserved total", "0 demo tokens");
    printField(
        "Terms hash",
        Buffer.from(createdPayment.termsHash).toString("hex")
    );

    /*
     * STAGE 5A
     * Record the first approval and prove that it is insufficient.
     */
    await beginStage(
        "STAGE 5A: FIRST APPROVAL",
        "Approver One signs. The payment must remain pending because the threshold is two."
    );

    const approvalOne = findApprovalPda(
        organization,
        paymentId,
        INITIAL_PAYMENT_REVISION,
        approverOneMemberId,
        new BN(INITIAL_AUTHORIZATION_REVISION)
    );

    const approvalOneInstruction = await program.methods
        .approvePayment(paymentId)
        .accountsStrict({
            authority: approverOne.publicKey,
            organization,
            approverMember: approverOneMember,
            vaultState,
            mint,
            vaultAta,
            recipient,
            approvalPolicyVersion: policyVersion,
            payment,
            approval: approvalOne,
            systemProgram: SystemProgram.programId,
        })
        .instruction();

    await sendInstruction(
        connection,
        "[5.1] Record first approval",
        approvalOneInstruction,
        approverOne
    );

    const paymentAfterFirstApproval = await program.account.payment.fetch(
        payment
    );

    const vaultAfterFirstApproval = await program.account.vaultState.fetch(
        vaultState
    );

    if (!hasVariant(paymentAfterFirstApproval.paymentState, "pendingApproval")) {
        throw new Error("One approval should not satisfy a two-of-two policy.");
    }

    if (!hasVariant(paymentAfterFirstApproval.reservationState, "none")) {
        throw new Error("One approval should not create a vault reservation.");
    }

    if (!vaultAfterFirstApproval.reservedTotal.isZero()) {
        throw new Error(
            "Vault reserved_total must remain zero after one approval."
        );
    }

    console.log("");
    console.log("FIRST APPROVAL RECORDED");
    console.log("");

    printField("Approval One PDA", approvalOne.toBase58());
    printField("Valid approvals", "1 of 2");
    printField("Payment state", "PendingApproval");
    printField("Reservation state", "None");
    printField("Vault reserved total", "0 demo tokens");
    printField("Can execute", false);

    /*
     * STAGE 5B
     * Record the second approval.
     *
     * Approval One and Approver One are supplied as remaining-account
     * witnesses. The program re-derives and validates both accounts
     * before counting the earlier approval.
     */
    await beginStage(
        "STAGE 5B: SECOND APPROVAL AND RESERVATION",
        "Approver Two signs. The program revalidates Approval One and atomically reserves 1,000 tokens."
    );

    const approvalTwo = findApprovalPda(
        organization,
        paymentId,
        INITIAL_PAYMENT_REVISION,
        approverTwoMemberId,
        new BN(INITIAL_AUTHORIZATION_REVISION)
    );

    const approvalTwoInstruction = await program.methods
        .approvePayment(paymentId)
        .accountsStrict({
            authority: approverTwo.publicKey,
            organization,
            approverMember: approverTwoMember,
            vaultState,
            mint,
            vaultAta,
            recipient,
            approvalPolicyVersion: policyVersion,
            payment,
            approval: approvalTwo,
            systemProgram: SystemProgram.programId,
        })
        .remainingAccounts([
            readonlyAccount(approvalOne),
            readonlyAccount(approverOneMember),
        ])
        .instruction();

    await sendInstruction(
        connection,
        "[5.2] Record second approval and reserve funds",
        approvalTwoInstruction,
        approverTwo
    );

    const approvedPayment = await program.account.payment.fetch(payment);

    const reservedVault = await program.account.vaultState.fetch(vaultState);

    const vaultTokenAccountAtReservation = await getAccount(connection, vaultAta);

    if (!hasVariant(approvedPayment.paymentState, "approved")) {
        throw new Error("Payment did not enter Approved state.");
    }

    if (!hasVariant(approvedPayment.reservationState, "held")) {
        throw new Error("Payment reservation did not enter Held state.");
    }

    if (!reservedVault.reservedTotal.eq(new BN(PAYMENT_AMOUNT.toString()))) {
        throw new Error("Vault reserved_total does not equal the payment amount.");
    }

    const reservedBaseUnits = BigInt(reservedVault.reservedTotal.toString());

    const withdrawableBaseUnits =
        vaultTokenAccountAtReservation.amount - reservedBaseUnits;

    console.log("");
    console.log("APPROVAL THRESHOLD REACHED");
    console.log("");

    printField("Approval Two PDA", approvalTwo.toBase58());
    printField("Valid approvals", "2 of 2");
    printField("Payment state", "Approved");
    printField("Reservation state", "Held");
    printField(
        "Vault token balance",
        `${formatTokenAmount(vaultTokenAccountAtReservation.amount)} demo tokens`
    );
    printField(
        "Vault reserved total",
        `${formatTokenAmount(reservedBaseUnits)} demo tokens`
    );
    printField(
        "Withdrawable balance",
        `${formatTokenAmount(withdrawableBaseUnits)} demo tokens`
    );
    printField("Can execute", true);

    /*
     * STAGE 6
     * A separately registered executor settles the approved payment.
     */
    await beginStage(
        "STAGE 6: EXECUTE SPL PAYMENT",
        "The registered executor transfers 1,000 tokens and atomically consumes the reservation."
    );

    const executeInstruction = await program.methods
        .executeSplPayment(paymentId)
        .accountsStrict({
            authority: executor.publicKey,
            organization,
            executorMember,
            mint,
            vaultState,
            vaultAta,
            recipient,
            destinationAta: destinationAta.address,
            payment,
            tokenProgram: TOKEN_PROGRAM_ID,
        })
        .instruction();

    const executionSignature = await sendInstruction(
        connection,
        "[6.1] Execute SPL payment",
        executeInstruction,
        executor
    );

    const finalPayment = await program.account.payment.fetch(payment);

    const finalVault = await program.account.vaultState.fetch(vaultState);

    const finalVaultTokenAccount = await getAccount(connection, vaultAta);

    const finalDestinationTokenAccount = await getAccount(
        connection,
        destinationAta.address
    );

    if (!hasVariant(finalPayment.paymentState, "paid")) {
        throw new Error("Payment did not enter Paid state.");
    }

    if (!hasVariant(finalPayment.reservationState, "consumed")) {
        throw new Error("Payment reservation did not enter Consumed state.");
    }

    if (!finalVault.reservedTotal.isZero()) {
        throw new Error("Vault reserved_total was not cleared after execution.");
    }

    if (finalDestinationTokenAccount.amount !== PAYMENT_AMOUNT) {
        throw new Error(
            "Recipient token balance does not equal the payment amount."
        );
    }

    if (finalVaultTokenAccount.amount !== VAULT_FUNDING_AMOUNT - PAYMENT_AMOUNT) {
        throw new Error(
            "Vault token balance did not decrease by the payment amount."
        );
    }

    console.log("");
    console.log("=".repeat(78));
    console.log("PAYMENT EXECUTED AND VERIFIED");
    console.log("=".repeat(78));
    console.log("");

    printField("Payment PDA", payment.toBase58());
    printField("Payment state", "Paid");
    printField("Reservation state", "Consumed");
    printField(
        "Vault token balance",
        `${formatTokenAmount(finalVaultTokenAccount.amount)} demo tokens`
    );
    printField("Vault reserved total", "0 demo tokens");
    printField(
        "Recipient token balance",
        `${formatTokenAmount(finalDestinationTokenAccount.amount)} demo tokens`
    );
    printField("Executor Member", executorMember.toBase58());
    printField("Execution signature", executionSignature);

    console.log("");
    console.log(`Explorer: ${explorerUrl(executionSignature)}`);
    console.log("");
    console.log("DEVNET GOVERNED PAYMENT DEMO PASSED");
}

main()
    .then(() => {
        presenter.close();

        /*
         * Flush piped output before exiting. Explicit termination prevents
         * leftover RPC polling or websocket activity from encountering a
         * later rate limit after successful final verification.
         */
        process.stdout.write("", () => {
            process.exit(0);
        });
    })
    .catch((error: unknown) => {
        presenter.close();

        console.error("");
        console.error("DEVNET GOVERNED PAYMENT DEMO FAILED");
        console.error(error);

        process.stderr.write("", () => {
            process.exit(1);
        });
    });