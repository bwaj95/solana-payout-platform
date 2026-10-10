import { readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

import { BN } from "@anchor-lang/core";
import {
    Connection,
    Keypair,
    sendAndConfirmTransaction,
    SystemProgram,
    Transaction,
} from "@solana/web3.js";

import {
    createProgram,
    DEVNET_RPC_URL,
    PROGRAM_ID,
} from "../clients/solana/program";

import {
    findMemberPda,
    findMemberWalletPda,
    findOrganizationPda,
} from "../clients/solana/pdas";

function loadDefaultWallet(): Keypair {
    /*
     * The secret key remains local. We read the same wallet configured in
     * Anchor.toml and never print or copy its secret bytes.
     */
    const walletPath = join(homedir(), ".config", "solana", "id.json");

    const secretKey = JSON.parse(readFileSync(walletPath, "utf8")) as number[];

    return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}

function createUniqueU64(): BN {
    /*
     * Date plus a three-digit random suffix makes repeated smoke-test runs
     * derive different organization PDAs.
     */
    const suffix = Math.floor(Math.random() * 1_000)
        .toString()
        .padStart(3, "0");

    return new BN(`${Date.now()}${suffix}`);
}

async function main(): Promise<void> {
    const connection = new Connection(DEVNET_RPC_URL, "confirmed");
    const program = createProgram(connection);
    const creator = loadDefaultWallet();

    console.log("RPC:", DEVNET_RPC_URL);
    console.log("Program:", PROGRAM_ID.toBase58());
    console.log("Creator:", creator.publicKey.toBase58());

    const deployedProgram = await connection.getAccountInfo(
        PROGRAM_ID,
        "confirmed"
    );

    if (!deployedProgram?.executable) {
        throw new Error("The payout program is not executable on devnet");
    }

    const organizationId = createUniqueU64();
    const ownerMemberId = new BN(1);

    const organization = findOrganizationPda(creator.publicKey, organizationId);

    const ownerMember = findMemberPda(organization, ownerMemberId);

    const ownerMemberWallet = findMemberWalletPda(
        organization,
        creator.publicKey
    );

    console.log("Organization ID:", organizationId.toString());
    console.log("Organization PDA:", organization.toBase58());
    console.log("Owner Member PDA:", ownerMember.toBase58());

    const initializeInstruction = await program.methods
        .initializeOrganization(organizationId, ownerMemberId)
        .accountsStrict({
            creator: creator.publicKey,
            organization: organization,
            ownerMember: ownerMember,
            ownerMemberWallet: ownerMemberWallet,
            systemProgram: SystemProgram.programId,
        })
        .instruction();

    const transaction = new Transaction().add(initializeInstruction);

    const signature = await sendAndConfirmTransaction(
        connection,
        transaction,
        [creator],
        {
            commitment: "confirmed",
        }
    );

    const storedOrganization = await program.account.organization.fetch(
        organization
    );

    const storedOwnerMember = await program.account.member.fetch(ownerMember);

    if (!storedOrganization.creator.equals(creator.publicKey)) {
        throw new Error("Stored organization creator does not match");
    }

    if (!storedOrganization.ownerMember.equals(ownerMember)) {
        throw new Error("Stored owner member does not match");
    }

    if (storedOrganization.paused) {
        throw new Error("New organization should not be paused");
    }

    if (!storedOwnerMember.authorizedWallet.equals(creator.publicKey)) {
        throw new Error("Stored owner wallet does not match");
    }

    if (!storedOwnerMember.active) {
        throw new Error("Owner member should be active");
    }

    console.log("Transaction:", signature);
    console.log(
        "Explorer:",
        `https://explorer.solana.com/tx/${signature}?cluster=devnet`
    );
    console.log("Stored organization verified");
    console.log("Stored owner member verified");
    console.log("DEVNET SMOKE TEST PASSED");
}

main().catch((error: unknown) => {
    console.error("DEVNET SMOKE TEST FAILED");
    console.error(error);
    process.exit(1);
});
