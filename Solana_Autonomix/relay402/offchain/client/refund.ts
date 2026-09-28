import { TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { Connection, PublicKey } from "@solana/web3.js";
import path from "path";
import { fetchConfig, fetchReceipt, isPending } from "../shared/accounts";
import { KEYS_DIR, env, loadKeypair, rpcUrlFromEnv } from "../shared/env";
import { configPda } from "../shared/pdas";
import { getProgram } from "../shared/program";

// Usage: npm run client:refund -- <receipt>
async function main() {
  const [receiptArg] = process.argv.slice(2);
  if (!receiptArg) throw new Error("usage: client:refund -- <receipt>");

  const connection = new Connection(rpcUrlFromEnv(), "confirmed");
  const client = loadKeypair(env("CLIENT_KEYPAIR", path.join(KEYS_DIR, "client.json")));
  const program = getProgram(connection, client);
  const receiptKey = new PublicKey(receiptArg);
  const receipt = await fetchReceipt(program, receiptKey);
  if (!receipt) throw new Error("receipt not found");
  if (!isPending(receipt)) throw new Error("receipt is not pending");
  const now = Math.floor(Date.now() / 1000);
  if (now < receipt.expiresAt.toNumber()) {
    throw new Error(`receipt expires in ${receipt.expiresAt.toNumber() - now}s, refund after that`);
  }
  const config = await fetchConfig(program, configPda(program.programId));
  if (!config) throw new Error("config not found");

  const sig = await program.methods
    .refundPayment()
    .accountsStrict({
      client: client.publicKey,
      config: configPda(program.programId),
      agent: receipt.agent,
      receipt: receiptKey,
      mint: config.mint,
      clientToken: getAssociatedTokenAddressSync(config.mint, client.publicKey),
      vault: config.vault,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .signers([client])
    .rpc({ commitment: "confirmed" });
  console.log(`refunded: ${sig}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
