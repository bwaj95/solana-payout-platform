import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

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
import {
  createProgram,
  DEVNET_RPC_URL,
  PROGRAM_ID,
} from "../clients/solana/program";

const MINT_DECIMALS = 6;
const VAULT_FUNDING_AMOUNT = 10_000_000n;
const PAYMENT_AMOUNT = 2_500_000n;
const TEMPORARY_SIGNER_FUNDING_LAMPORTS = 10_000_000;

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
    { commitment: "confirmed" }
  );

  console.log(`${label}: ${signature}`);
  console.log(`  ${explorerUrl(signature)}`);

  return signature;
}

function readonlyAccount(pubkey: PublicKey): AccountMeta {
  return { pubkey, isWritable: false, isSigner: false };
}

function hasVariant(value: object, variant: string): boolean {
  return variant in value;
}

async function main(): Promise<void> {
  const connection = new Connection(DEVNET_RPC_URL, "confirmed");
  const program = createProgram(connection);
  const creator = loadDefaultWallet();

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
  const vaultState = findVaultStatePda(organization, vaultId);
  const recipient = findRecipientPda(organization, recipientId);
  const policyVersion = findPolicyVersionPda(
    organization,
    policyId,
    policyVersionNumber
  );
  const payment = findPaymentPda(organization, paymentId);

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

  console.log("RPC:", DEVNET_RPC_URL);
  console.log("Program:", PROGRAM_ID.toBase58());
  console.log("Creator:", creator.publicKey.toBase58());
  console.log("Organization ID:", organizationId.toString());
  console.log("Organization PDA:", organization.toBase58());

  const deployedProgram = await connection.getAccountInfo(
    PROGRAM_ID,
    "confirmed"
  );
  if (!deployedProgram?.executable) {
    throw new Error("The payout program is not executable on devnet");
  }

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
    "1. Initialize organization",
    initializeOrganizationInstruction,
    creator
  );

  /*
   * Approvers pay rent for their Approval PDAs, and the executor pays the
   * execution transaction fee. These transfers make the generated wallets
   * usable without requesting separate devnet airdrops.
   */
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
    { commitment: "confirmed" }
  );
  console.log("2. Fund temporary signers:", fundingSignature);
  console.log(`  ${explorerUrl(fundingSignature)}`);

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
      `3. Create ${memberDefinition.label}`,
      createMemberInstruction,
      creator
    );
  }

  const mint = await createMint(
    connection,
    creator,
    creator.publicKey,
    null,
    MINT_DECIMALS,
    undefined,
    { commitment: "confirmed" }
  );
  console.log("4. Create demo mint:", mint.toBase58());

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
    "5. Initialize SPL vault",
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
    { commitment: "confirmed" }
  );
  console.log("6. Fund vault with demo tokens:", mintToSignature);
  console.log(`  ${explorerUrl(mintToSignature)}`);

  const destinationAta = await getOrCreateAssociatedTokenAccount(
    connection,
    creator,
    mint,
    recipientWallet.publicKey,
    false,
    "confirmed"
  );
  console.log("7. Recipient ATA:", destinationAta.address.toBase58());

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
    "8. Register recipient",
    registerRecipientInstruction,
    creator
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
    "9. Create two-of-two approval policy",
    createPolicyInstruction,
    creator
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
    "10. Create payment",
    createPaymentInstruction,
    creator
  );

  const approvalOne = findApprovalPda(
    organization,
    paymentId,
    INITIAL_PAYMENT_REVISION,
    approverOneMemberId,
    new BN(INITIAL_AUTHORIZATION_REVISION)
  );
  const approvalTwo = findApprovalPda(
    organization,
    paymentId,
    INITIAL_PAYMENT_REVISION,
    approverTwoMemberId,
    new BN(INITIAL_AUTHORIZATION_REVISION)
  );

  const approveOneInstruction = await program.methods
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
    "11. Record first approval",
    approveOneInstruction,
    approverOne
  );

  /*
   * The program does not scan Solana for older approvals. The client passes
   * each earlier Approval + Member pair as witnesses. The program then
   * revalidates both accounts before counting them.
   */
  const approveTwoInstruction = await program.methods
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
    "12. Record second approval and reserve funds",
    approveTwoInstruction,
    approverTwo
  );

  const approvedPayment = await program.account.payment.fetch(payment);
  const reservedVault = await program.account.vaultState.fetch(vaultState);

  if (!hasVariant(approvedPayment.paymentState, "approved")) {
    throw new Error("Payment did not enter Approved state");
  }
  if (!hasVariant(approvedPayment.reservationState, "held")) {
    throw new Error("Payment reservation was not held");
  }
  if (!reservedVault.reservedTotal.eq(new BN(PAYMENT_AMOUNT.toString()))) {
    throw new Error("Vault reserved_total does not equal payment amount");
  }
  console.log("  Approved + Held state verified before execution");

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
    "13. Execute SPL payment",
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
    throw new Error("Payment did not enter Paid state");
  }
  if (!hasVariant(finalPayment.reservationState, "consumed")) {
    throw new Error("Payment reservation was not consumed");
  }
  if (!finalVault.reservedTotal.isZero()) {
    throw new Error("Vault reserved_total was not released after execution");
  }
  if (finalDestinationTokenAccount.amount !== PAYMENT_AMOUNT) {
    throw new Error("Recipient token balance does not equal payment amount");
  }
  if (finalVaultTokenAccount.amount !== VAULT_FUNDING_AMOUNT - PAYMENT_AMOUNT) {
    throw new Error("Vault token balance did not decrease correctly");
  }

  console.log("");
  console.log("FINAL ON-CHAIN STATE");
  console.log("Payment PDA:", payment.toBase58());
  console.log("Payment state: Paid");
  console.log("Reservation state: Consumed");
  console.log("Vault reserved total: 0");
  console.log(
    "Recipient amount:",
    finalDestinationTokenAccount.amount.toString()
  );
  console.log("Execution transaction:", executionSignature);
  console.log("DEVNET END-TO-END PAYMENT FLOW PASSED");
}

main().catch((error: unknown) => {
  console.error("DEVNET END-TO-END PAYMENT FLOW FAILED");
  console.error(error);
  process.exit(1);
});
