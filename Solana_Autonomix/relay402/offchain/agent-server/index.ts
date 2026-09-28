import { Connection, PublicKey } from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { fetchAgent, fetchConfig } from "../shared/accounts";
import {
  KEYS_DIR,
  ROOT_DIR,
  env,
  envInt,
  loadKeypair,
  networkFromEnv,
  readDeployment,
  rpcUrlFromEnv,
} from "../shared/env";
import { getJson } from "../shared/http";
import { agentPda, configPda } from "../shared/pdas";
import { getProgram } from "../shared/program";
import { sha256Hex } from "../shared/x402";
import { createAgentApp } from "./server";

async function main() {
  const network = networkFromEnv();
  const deployment = readDeployment();
  const connection = new Connection(rpcUrlFromEnv(), "confirmed");
  const operator = loadKeypair(env("OPERATOR_KEYPAIR", path.join(KEYS_DIR, "operator.json")));
  const program = getProgram(connection, operator);

  const agentId = BigInt(env("AGENT_ID", deployment.agentId));
  const agentKey = agentPda(agentId, program.programId);
  const agent = await fetchAgent(program, agentKey);
  const config = await fetchConfig(program, configPda(program.programId));
  if (!agent || !config) throw new Error("agent or config not found on-chain; run setup first");

  // Refuse to start with the wrong key instead of failing on every settlement.
  if (!agent.operator.equals(operator.publicKey)) {
    throw new Error(`operator key ${operator.publicKey.toBase58()} is not the agent operator`);
  }

  const metadataFile = env("AGENT_METADATA_FILE", path.join(ROOT_DIR, "agent-metadata.json"));
  const metadata = fs.readFileSync(metadataFile);
  if (sha256Hex(metadata) !== Buffer.from(agent.metadataHash).toString("hex")) {
    console.warn("[agent] WARNING: metadata file hash does not match the on-chain metadata_hash");
  }

  const facilitatorUrl = env("FACILITATOR_URL", "http://127.0.0.1:4020");
  const supported = (await getJson(`${facilitatorUrl}/supported`, 5000)) as {
    kinds: Array<{ scheme: string; network: string; extra: { feePayer: string; programId: string } }>;
  };
  const kind = supported.kinds.find((k) => k.scheme === "relay402-escrow" && k.network === network);
  if (!kind) throw new Error("facilitator does not support relay402-escrow on this network");
  if (kind.extra.programId !== program.programId.toBase58()) {
    throw new Error("facilitator is configured for a different program id");
  }

  const publicBaseUrl = env("AGENT_PUBLIC_URL", deployment.agentPublicUrl).replace(/\/+$/, "");
  const app = createAgentApp({
    program,
    network,
    agentId,
    agentKey,
    operator,
    mint: config.mint,
    facilitatorUrl,
    facilitatorFeePayer: new PublicKey(kind.extra.feePayer),
    publicBaseUrl,
    description: "Relay402 demo agent: answers a prompt",
    maxTimeoutSeconds: envInt("MAX_TIMEOUT_SECONDS", 120),
    taskTimeoutMs: envInt("TASK_TIMEOUT_MS", 60_000),
    facilitatorTimeoutMs: envInt("FACILITATOR_TIMEOUT_MS", 90_000),
    metadata,
  });

  const port = envInt("AGENT_PORT", 4021);
  const host = env("AGENT_HOST", "127.0.0.1");
  app.listen(port, host, () => {
    console.log(`[agent] listening on http://${host}:${port} (public: ${publicBaseUrl})`);
    console.log(`[agent] agent #${agentId} ${agentKey.toBase58()} price=${agent.price.toString()}`);
  });
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
