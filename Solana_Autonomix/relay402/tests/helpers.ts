import * as anchor from "@coral-xyz/anchor";
import { AnchorProvider, BN, Program, Wallet } from "@coral-xyz/anchor";
import {
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccount,
  getAccount,
  getAssociatedTokenAddressSync,
  mintTo,
} from "@solana/spl-token";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import { expect } from "chai";
import idl from "../target/idl/relay402.json";
import type { Relay402 } from "../target/types/relay402";
import { agentPda, configPda, receiptPda, vaultPda } from "../offchain/shared/pdas";

// Confirm at "confirmed" so reads right after a transaction see its effects.
const envProvider = AnchorProvider.env();
export const provider = new AnchorProvider(
  new Connection(envProvider.connection.rpcEndpoint, "confirmed"),
  envProvider.wallet,
  { commitment: "confirmed", preflightCommitment: "confirmed" },
);
anchor.setProvider(provider);
export const connection: Connection = provider.connection;
export const program = new Program<Relay402>(idl as Relay402, provider);
export const admin: Keypair = (provider.wallet as Wallet).payer;
export const PROGRAM_ID = program.programId;
export const CONFIG = configPda(PROGRAM_ID);
export const VAULT = vaultPda(PROGRAM_ID);
export const PROGRAM_DATA = PublicKey.findProgramAddressSync(
  [PROGRAM_ID.toBuffer()],
  new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111"),
)[0];

export async function airdrop(to: PublicKey, sol = 5): Promise<void> {
  const sig = await connection.requestAirdrop(to, sol * LAMPORTS_PER_SOL);
  const bh = await connection.getLatestBlockhash("confirmed");
  await connection.confirmTransaction({ signature: sig, ...bh }, "confirmed");
}

export async function newUser(sol = 5): Promise<Keypair> {
  const kp = Keypair.generate();
  await airdrop(kp.publicKey, sol);
  return kp;
}

export async function ata(mint: PublicKey, owner: PublicKey): Promise<PublicKey> {
  const address = getAssociatedTokenAddressSync(mint, owner, true);
  const info = await connection.getAccountInfo(address);
  if (!info) await createAssociatedTokenAccount(connection, admin, mint, owner);
  return address;
}

export async function fund(mint: PublicKey, owner: PublicKey, amount: bigint): Promise<PublicKey> {
  const address = await ata(mint, owner);
  await mintTo(connection, admin, mint, address, admin, amount);
  return address;
}

export async function tokenBalance(address: PublicKey): Promise<bigint> {
  return (await getAccount(connection, address, "confirmed")).amount;
}

export const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Wait until the cluster clock (used by the program) reaches `ts`. */
export async function waitForChainTime(ts: number): Promise<void> {
  for (;;) {
    const slot = await connection.getSlot("confirmed");
    const t = await connection.getBlockTime(slot);
    if (t !== null && t >= ts) return;
    await sleep(1000);
  }
}

/** Assert a promise rejects and its error or logs mention `needle`. */
export async function expectError(p: Promise<unknown>, needle: string): Promise<void> {
  let err: unknown;
  try {
    await p;
  } catch (e) {
    err = e;
  }
  expect(err, `expected failure containing "${needle}"`).to.not.equal(undefined);
  const e = err as { logs?: string[]; message?: string; transactionLogs?: string[] };
  const text = [String(err), e.message ?? "", ...(e.logs ?? []), ...(e.transactionLogs ?? [])].join("\n");
  expect(text, text).to.include(needle);
}

export async function fetchConfigState() {
  return program.account.config.fetch(CONFIG, "confirmed");
}

// ------------------------------------------------------------------
// Instruction wrappers (explicit accounts everywhere)
// ------------------------------------------------------------------

export async function registerAgent(opts: {
  owner: Keypair;
  operator: PublicKey;
  payout: PublicKey;
  price: number | bigint;
  endpoint?: string;
  metadataUri?: string;
  metadataHash?: number[];
}): Promise<{ id: bigint; agent: PublicKey }> {
  const config = await fetchConfigState();
  const id = BigInt(config.nextAgentId.toString());
  const agent = agentPda(id, PROGRAM_ID);
  await program.methods
    .registerAgent({
      endpoint: opts.endpoint ?? "https://agent.example",
      metadataUri: opts.metadataUri ?? "https://agent.example/metadata.json",
      metadataHash: opts.metadataHash ?? new Array(32).fill(7),
      price: new BN(opts.price.toString()),
      operator: opts.operator,
    })
    .accountsStrict({
      owner: opts.owner.publicKey,
      config: CONFIG,
      agent,
      payout: opts.payout,
      systemProgram: SystemProgram.programId,
    })
    .signers([opts.owner])
    .rpc();
  return { id, agent };
}

export async function createPayment(opts: {
  client: Keypair;
  agent: PublicKey;
  mint: PublicKey;
  nonce: bigint;
  windowSecs?: number;
  maxAmount?: bigint;
  clientToken?: PublicKey;
}): Promise<PublicKey> {
  const receipt = receiptPda(opts.agent, opts.client.publicKey, opts.nonce, PROGRAM_ID);
  await program.methods
    .createPayment(
      new BN(opts.nonce.toString()),
      new BN(opts.windowSecs ?? 3600),
      new BN((opts.maxAmount ?? 10n ** 12n).toString()),
    )
    .accountsStrict({
      client: opts.client.publicKey,
      config: CONFIG,
      agent: opts.agent,
      receipt,
      mint: opts.mint,
      clientToken: opts.clientToken ?? getAssociatedTokenAddressSync(opts.mint, opts.client.publicKey),
      vault: VAULT,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .signers([opts.client])
    .rpc();
  return receipt;
}

export async function settle(opts: {
  operator: Keypair;
  agent: PublicKey;
  receipt: PublicKey;
  mint: PublicKey;
  payout: PublicKey;
  treasury: PublicKey;
}): Promise<string> {
  return program.methods
    .settlePayment()
    .accountsStrict({
      operator: opts.operator.publicKey,
      config: CONFIG,
      payout: opts.payout,
      agent: opts.agent,
      receipt: opts.receipt,
      mint: opts.mint,
      vault: VAULT,
      treasury: opts.treasury,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .signers([opts.operator])
    .rpc();
}

export async function refund(opts: {
  client: Keypair;
  agent: PublicKey;
  receipt: PublicKey;
  mint: PublicKey;
  clientToken?: PublicKey;
}): Promise<string> {
  return program.methods
    .refundPayment()
    .accountsStrict({
      client: opts.client.publicKey,
      config: CONFIG,
      agent: opts.agent,
      receipt: opts.receipt,
      mint: opts.mint,
      clientToken: opts.clientToken ?? getAssociatedTokenAddressSync(opts.mint, opts.client.publicKey),
      vault: VAULT,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .signers([opts.client])
    .rpc();
}

export async function feedback(opts: {
  client: Keypair;
  agent: PublicKey;
  receipt: PublicKey;
  score: number;
}): Promise<string> {
  return program.methods
    .submitFeedback(opts.score)
    .accountsStrict({ client: opts.client.publicKey, agent: opts.agent, receipt: opts.receipt })
    .signers([opts.client])
    .rpc();
}

export async function updateConfig(
  signer: Keypair,
  args: { feeBps?: number; minPrice?: number; paused?: boolean },
): Promise<string> {
  return program.methods
    .updateConfig(
      args.feeBps ?? null,
      args.minPrice === undefined ? null : new BN(args.minPrice),
      args.paused ?? null,
    )
    .accountsStrict({ admin: signer.publicKey, config: CONFIG })
    .signers([signer])
    .rpc();
}
