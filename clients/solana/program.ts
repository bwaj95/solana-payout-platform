import { Program } from "@anchor-lang/core";
import { Connection, PublicKey } from "@solana/web3.js";

import idl from "./idl/solana_payout_platform.json";
import type { SolanaPayoutPlatform } from "./idl/solana_payout_platform";

export const DEVNET_RPC_URL = "https://api.devnet.solana.com";

export const PROGRAM_ID = new PublicKey(idl.address);

export function createProgram(
  connection: Connection
): Program<SolanaPayoutPlatform> {
  /*
   * A connection-only Program can build instructions and fetch accounts.
   *
   * We submit the final transaction separately with the local devnet
   * keypair. In the frontend, the connected browser wallet will perform
   * that signing step instead.
   */
  return new Program(idl as SolanaPayoutPlatform, {
    connection,
  });
}
