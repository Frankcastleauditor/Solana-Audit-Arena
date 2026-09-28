import { Connection } from "@solana/web3.js";
import path from "path";
import { KEYS_DIR, env, loadKeypair, networkFromEnv, readDeployment, rpcUrlFromEnv } from "../shared/env";
import { getProgram } from "../shared/program";
import { paidCall } from "./lib";

// Usage: npm run client:call -- "your prompt here"
async function main() {
  const prompt = process.argv.slice(2).join(" ") || "Say hello in five words.";
  const deployment = readDeployment();
  const network = networkFromEnv();
  const connection = new Connection(rpcUrlFromEnv(), "confirmed");
  const client = loadKeypair(env("CLIENT_KEYPAIR", path.join(KEYS_DIR, "client.json")));
  const program = getProgram(connection, client);
  const url = env("AGENT_URL", `${deployment.agentPublicUrl.replace(/\/+$/, "")}/api/run`);
  const maxAmount = BigInt(env("MAX_AMOUNT", "50000"));

  const result = await paidCall({
    program,
    client,
    network,
    url,
    body: JSON.stringify({ prompt }),
    maxAmount,
  });

  console.log(`status: ${result.status}`);
  console.log("body:", JSON.stringify(result.body, null, 2));
  if (result.paymentResponse) console.log("settlement:", result.paymentResponse);
  if (result.receipt) {
    console.log(`receipt: ${result.receipt.toBase58()}`);
    if (result.status === 200) {
      console.log(`rate it:  npm run client:feedback -- ${result.receipt.toBase58()} 5`);
    } else {
      console.log(`not settled. After expiry run: npm run client:refund -- ${result.receipt.toBase58()}`);
    }
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
