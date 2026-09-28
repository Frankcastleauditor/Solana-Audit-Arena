import type { IdlAccounts, Program } from "@coral-xyz/anchor";
import { Commitment, PublicKey } from "@solana/web3.js";
import type { Relay402 } from "./program";

export type ConfigAccount = IdlAccounts<Relay402>["config"];
export type AgentAccount = IdlAccounts<Relay402>["agent"];
export type ReceiptAccount = IdlAccounts<Relay402>["receipt"];

/** Anchor 0.31 coder keys accounts by their camelCase IDL name. */
type AccountName = "config" | "agent" | "receipt";

/**
 * Fetch and decode a program account. Checks the owner explicitly (the
 * Anchor TS client only checks the discriminator) and returns null when the
 * account does not exist.
 */
async function fetchOwned<T>(
  program: Program<Relay402>,
  name: AccountName,
  address: PublicKey,
  commitment: Commitment,
): Promise<T | null> {
  const info = await program.provider.connection.getAccountInfo(address, commitment);
  if (info === null) return null;
  if (!info.owner.equals(program.programId)) {
    throw new Error(`${name} ${address.toBase58()} is not owned by the program`);
  }
  // decode() rejects a wrong discriminator.
  return program.coder.accounts.decode<T>(name, info.data);
}

export function fetchConfig(p: Program<Relay402>, a: PublicKey, c: Commitment = "confirmed") {
  return fetchOwned<ConfigAccount>(p, "config", a, c);
}

export function fetchAgent(p: Program<Relay402>, a: PublicKey, c: Commitment = "confirmed") {
  return fetchOwned<AgentAccount>(p, "agent", a, c);
}

export function fetchReceipt(p: Program<Relay402>, a: PublicKey, c: Commitment = "confirmed") {
  return fetchOwned<ReceiptAccount>(p, "receipt", a, c);
}

export function isPending(receipt: ReceiptAccount): boolean {
  return "pending" in receipt.status;
}

export function isSettled(receipt: ReceiptAccount): boolean {
  return "settled" in receipt.status;
}
