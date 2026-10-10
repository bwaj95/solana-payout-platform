import { BN } from "@anchor-lang/core";
import { PublicKey } from "@solana/web3.js";
import { PROGRAM_ID } from "./program";

export const ORGANIZATION_SEED = Buffer.from("organization");
export const MEMBER_SEED = Buffer.from("member");
export const MEMBER_WALLET_SEED = Buffer.from("member_wallet");
export const VAULT_SEED = Buffer.from("vault");
export const RECIPIENT_SEED = Buffer.from("recipient");
export const POLICY_SEED = Buffer.from("policy");
export const PAYMENT_SEED = Buffer.from("payment");
export const APPROVAL_SEED = Buffer.from("approval");

export function u64ToLeBytes(value: BN): Buffer {
  return value.toArrayLike(Buffer, "le", 8);
}

export function u32ToLeBytes(value: number): Buffer {
  const bytes = Buffer.alloc(4);
  bytes.writeUInt32LE(value);
  return bytes;
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

export function findVaultStatePda(
  organization: PublicKey,
  vaultId: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [VAULT_SEED, organization.toBuffer(), u64ToLeBytes(vaultId)],
    PROGRAM_ID
  )[0];
}

export function findRecipientPda(
  organization: PublicKey,
  recipientId: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [RECIPIENT_SEED, organization.toBuffer(), u64ToLeBytes(recipientId)],
    PROGRAM_ID
  )[0];
}

export function findPolicyVersionPda(
  organization: PublicKey,
  policyId: BN,
  version: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [
      POLICY_SEED,
      organization.toBuffer(),
      u64ToLeBytes(policyId),
      u64ToLeBytes(version),
    ],
    PROGRAM_ID
  )[0];
}

export function findPaymentPda(
  organization: PublicKey,
  paymentId: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [PAYMENT_SEED, organization.toBuffer(), u64ToLeBytes(paymentId)],
    PROGRAM_ID
  )[0];
}

export function findApprovalPda(
  organization: PublicKey,
  paymentId: BN,
  paymentRevision: number,
  memberId: BN,
  memberAuthorizationRevision: BN
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [
      APPROVAL_SEED,
      organization.toBuffer(),
      u64ToLeBytes(paymentId),
      u32ToLeBytes(paymentRevision),
      u64ToLeBytes(memberId),
      u64ToLeBytes(memberAuthorizationRevision),
    ],
    PROGRAM_ID
  )[0];
}
