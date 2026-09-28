import { AnchorProvider, Program, Wallet } from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import idl from "../../target/idl/relay402.json";
import type { Relay402 } from "../../target/types/relay402";

export type { Relay402 };

export const PROGRAM_ID = new PublicKey(idl.address);

/**
 * Anchor client. The keypair is only used when the caller sends transactions
 * through `.rpc()`. Read-only users can omit it.
 */
export function getProgram(connection: Connection, keypair?: Keypair): Program<Relay402> {
  const wallet = new Wallet(keypair ?? Keypair.generate());
  const provider = new AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  return new Program(idl as Relay402, provider);
}
