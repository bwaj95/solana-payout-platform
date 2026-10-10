import { BN } from "@anchor-lang/core";
import { PublicKey } from "@solana/web3.js";
import { PROGRAM_ID } from "./program";

export const ORGANIZATION_SEED = Buffer.from("organization");
export const MEMBER_SEED = Buffer.from("member");
export const MEMBER_WALLET_SEED = Buffer.from("member_wallet");

export function u64ToLeBytes(value: BN): Buffer {
    return value.toArrayLike(Buffer, "le", 8);
}

export function findOrganizationPda(
    creator: PublicKey,
    organizationId: BN
): PublicKey {
    return PublicKey.findProgramAddressSync(
        [ORGANIZATION_SEED, creator.toBuffer(), u64ToLeBytes(organizationId)],
        PROGRAM_ID
    )[0];
}

export function findMemberPda(
    organization: PublicKey,
    memberId: BN
): PublicKey {
    return PublicKey.findProgramAddressSync(
        [MEMBER_SEED, organization.toBuffer(), u64ToLeBytes(memberId)],
        PROGRAM_ID
    )[0];
}

export function findMemberWalletPda(
    organization: PublicKey,
    authorizedWallet: PublicKey
): PublicKey {
    return PublicKey.findProgramAddressSync(
        [MEMBER_WALLET_SEED, organization.toBuffer(), authorizedWallet.toBuffer()],
        PROGRAM_ID
    )[0];
}
