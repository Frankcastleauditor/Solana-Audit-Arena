import { Connection, PublicKey } from "@solana/web3.js";
import path from "path";
import { fetchReceipt, isSettled } from "../shared/accounts";
import { KEYS_DIR, env, loadKeypair, rpcUrlFromEnv } from "../shared/env";
import { getProgram } from "../shared/program";

// Usage: npm run client:feedback -- <receipt> <score 1-5>
async function main() {
  const [receiptArg, scoreArg] = process.argv.slice(2);
  if (!receiptArg || !scoreArg) throw new Error("usage: client:feedback -- <receipt> <score 1-5>");
  const score = Number(scoreArg);
  if (!Number.isInteger(score) || score < 1 || score > 5) throw new Error("score must be 1..5");

  const connection = new Connection(rpcUrlFromEnv(), "confirmed");
  const client = loadKeypair(env("CLIENT_KEYPAIR", path.join(KEYS_DIR, "client.json")));
  const program = getProgram(connection, client);
  const receiptKey = new PublicKey(receiptArg);
  const receipt = await fetchReceipt(program, receiptKey);
  if (!receipt) throw new Error("receipt not found (already rated or closed?)");
  if (!isSettled(receipt)) throw new Error("receipt is not settled");

  const sig = await program.methods
    .submitFeedback(score)
    .accountsStrict({ client: client.publicKey, agent: receipt.agent, receipt: receiptKey })
    .signers([client])
    .rpc({ commitment: "confirmed" });
  console.log(`feedback submitted: ${sig}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
