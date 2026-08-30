import { PublicKey } from "@solana/web3.js";
import { keccak256 as ethersKeccak256 } from "ethers";

const DOMAIN_TAG = Buffer.from("solana-permit2:SignatureTransfer", "utf8");
const WITNESS_DOMAIN_TAG = Buffer.from("solana-x402:Exact:Witness", "utf8");

function u64LE(value: bigint | number): Buffer {
  const buf = Buffer.alloc(8);
  buf.writeBigUInt64LE(BigInt(value));
  return buf;
}

function keccak256(chunks: Buffer[]): Buffer {
  const hex = ethersKeccak256(Buffer.concat(chunks));
  return Buffer.from(hex.slice(2), "hex");
}

export interface PermitTransferFromArgs {
  permittedToken: PublicKey;
  permittedAmount: bigint;
  nonce: bigint;
  deadline: bigint;
}

export interface Witness {
  to: PublicKey;
  validAfter: bigint;
}

export function hashPermitWitnessTransferFrom(
  permit: PermitTransferFromArgs,
  spender: PublicKey,
  witnessHash: Buffer,
): Buffer {
  return keccak256([
    DOMAIN_TAG,
    Buffer.from("PermitWitnessTransferFrom", "utf8"),
    permit.permittedToken.toBuffer(),
    u64LE(permit.permittedAmount),
    spender.toBuffer(),
    u64LE(permit.nonce),
    u64LE(permit.deadline),
    witnessHash,
  ]);
}

export function hashPermitTransferFrom(permit: PermitTransferFromArgs, spender: PublicKey): Buffer {
  return keccak256([
    DOMAIN_TAG,
    Buffer.from("PermitTransferFrom", "utf8"),
    permit.permittedToken.toBuffer(),
    u64LE(permit.permittedAmount),
    spender.toBuffer(),
    u64LE(permit.nonce),
    u64LE(permit.deadline),
  ]);
}

export function hashWitness(witness: Witness): Buffer {
  return keccak256([WITNESS_DOMAIN_TAG, witness.to.toBuffer(), u64LE(witness.validAfter)]);
}
