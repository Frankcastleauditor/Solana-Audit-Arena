import { PublicKey } from "@solana/web3.js";
import { PROGRAM_ID } from "./program";

export const U64_MAX = (1n << 64n) - 1n;

export function u64Le(value: bigint): Buffer {
  if (value < 0n || value > U64_MAX) throw new Error("value out of u64 range");
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(value);
  return buf;
}

export function configPda(programId: PublicKey = PROGRAM_ID): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("config")], programId)[0];
}

export function vaultPda(programId: PublicKey = PROGRAM_ID): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("vault")], programId)[0];
}

export function agentPda(agentId: bigint, programId: PublicKey = PROGRAM_ID): PublicKey {
  return PublicKey.findProgramAddressSync([Buffer.from("agent"), u64Le(agentId)], programId)[0];
}

export function receiptPda(
  agent: PublicKey,
  client: PublicKey,
  nonce: bigint,
  programId: PublicKey = PROGRAM_ID,
): PublicKey {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("receipt"), agent.toBuffer(), client.toBuffer(), u64Le(nonce)],
    programId,
  )[0];
}
