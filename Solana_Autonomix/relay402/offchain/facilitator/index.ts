import { Connection } from "@solana/web3.js";
import path from "path";
import { KEYS_DIR, env, envInt, loadKeypair, networkFromEnv, rpcUrlFromEnv } from "../shared/env";
import { getProgram } from "../shared/program";
import { Facilitator } from "./facilitator";
import { createFacilitatorApp } from "./server";

async function main() {
  const network = networkFromEnv();
  const connection = new Connection(rpcUrlFromEnv(), "confirmed");
  const feePayer = loadKeypair(env("FACILITATOR_KEYPAIR", path.join(KEYS_DIR, "facilitator.json")));
  const program = getProgram(connection, feePayer);

  const facilitator = new Facilitator({ program, feePayer, network });
  const app = createFacilitatorApp(facilitator);
  const port = envInt("FACILITATOR_PORT", 4020);
  const host = env("FACILITATOR_HOST", "127.0.0.1");

  const balance = await connection.getBalance(feePayer.publicKey);
  app.listen(port, host, () => {
    console.log(`[facilitator] listening on http://${host}:${port}`);
    console.log(`[facilitator] network=${network} program=${program.programId.toBase58()}`);
    console.log(`[facilitator] fee payer=${feePayer.publicKey.toBase58()} balance=${balance / 1e9} SOL`);
  });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
