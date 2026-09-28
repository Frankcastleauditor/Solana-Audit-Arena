/**
 * One-shot setup for localnet or devnet:
 *   - creates role keypairs in .keys/ (agent owner, operator, client, facilitator)
 *   - funds them with SOL
 *   - creates a test USDC mint (localnet) or uses MINT (devnet USDC)
 *   - initializes the protocol config (admin = program upgrade authority)
 *   - writes agent-metadata.json and registers the demo agent
 *   - writes deployment.json for the services and the client
 * Safe to re-run: existing config and agent are reused.
 */
import { BN } from "@coral-xyz/anchor";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  getAccount,
  getOrCreateAssociatedTokenAccount,
  mintTo,
} from "@solana/spl-token";
import {
  Connection,
  Keypair,
  LAMPORTS_PER_SOL,
  PublicKey,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import fs from "fs";
import path from "path";
import { fetchAgent, fetchConfig } from "../shared/accounts";
import {
  DEPLOYMENT_FILE,
  Deployment,
  KEYS_DIR,
  ROOT_DIR,
  env,
  envInt,
  loadKeypair,
  loadOrCreateKeypair,
  networkFromEnv,
  rpcUrlFromEnv,
  writeDeployment,
} from "../shared/env";
import { agentPda, configPda, vaultPda } from "../shared/pdas";
import { getProgram } from "../shared/program";
import { sha256Hex } from "../shared/x402";

const USDC_DECIMALS = 6;
const BPF_LOADER_UPGRADEABLE_PROGRAM_ID = new PublicKey("BPFLoaderUpgradeab1e11111111111111111111111");

async function fundSol(connection: Connection, admin: Keypair, to: PublicKey, sol: number, local: boolean) {
  const target = sol * LAMPORTS_PER_SOL;
  const balance = await connection.getBalance(to);
  if (balance >= target / 2) return;
  if (local) {
    const sig = await connection.requestAirdrop(to, target);
    await connection.confirmTransaction(sig, "confirmed");
    return;
  }
  // devnet: move SOL from the admin wallet (airdrops are rate limited)
  const tx = new Transaction().add(
    SystemProgram.transfer({ fromPubkey: admin.publicKey, toPubkey: to, lamports: target - balance }),
  );
  await sendAndConfirmTransaction(connection, tx, [admin], { commitment: "confirmed" });
}

async function main() {
  const network = networkFromEnv();
  const local = network === "solana-localnet";
  if (network === "solana") throw new Error("setup script is for localnet/devnet only");
  const rpcUrl = rpcUrlFromEnv();
  const connection = new Connection(rpcUrl, "confirmed");

  const admin = loadKeypair(env("ADMIN_KEYPAIR", "~/.config/solana/id.json"));
  const program = getProgram(connection, admin);
  const programId = program.programId;
  console.log(`program:  ${programId.toBase58()}`);
  console.log(`admin:    ${admin.publicKey.toBase58()}`);

  const programInfo = await connection.getAccountInfo(programId);
  if (!programInfo || !programInfo.executable) {
    throw new Error("program is not deployed on this cluster. Run `anchor deploy` first.");
  }

  const owner = loadOrCreateKeypair(path.join(KEYS_DIR, "agent-owner.json"));
  const operator = loadOrCreateKeypair(path.join(KEYS_DIR, "operator.json"));
  const client = loadOrCreateKeypair(path.join(KEYS_DIR, "client.json"));
  const facilitator = loadOrCreateKeypair(path.join(KEYS_DIR, "facilitator.json"));

  if (local) await fundSol(connection, admin, admin.publicKey, 10, true);
  await fundSol(connection, admin, owner.publicKey, local ? 2 : 0.1, local);
  await fundSol(connection, admin, client.publicKey, local ? 2 : 0.1, local);
  await fundSol(connection, admin, facilitator.publicKey, local ? 2 : 0.1, local);

  // ---------------- mint + config ----------------
  const configKey = configPda(programId);
  let config = await fetchConfig(program, configKey);
  let mint: PublicKey;
  let createdTestMint = false;

  if (config) {
    mint = config.mint;
    console.log(`config exists, mint ${mint.toBase58()}`);
  } else if (process.env.MINT) {
    mint = new PublicKey(process.env.MINT);
  } else {
    if (!local) throw new Error("set MINT to the devnet USDC mint");
    mint = await createMint(connection, admin, admin.publicKey, null, USDC_DECIMALS);
    createdTestMint = true;
    console.log(`created test USDC mint ${mint.toBase58()}`);
  }

  const treasury = await getOrCreateAssociatedTokenAccount(connection, admin, mint, admin.publicKey);

  if (!config) {
    const [programData] = PublicKey.findProgramAddressSync(
      [programId.toBuffer()],
      BPF_LOADER_UPGRADEABLE_PROGRAM_ID,
    );
    await program.methods
      .initializeConfig(envInt("FEE_BPS", 100), new BN(env("MIN_PRICE", "1000")))
      .accountsStrict({
        admin: admin.publicKey,
        config: configKey,
        mint,
        vault: vaultPda(programId),
        treasury: treasury.address,
        program: programId,
        programData,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc({ commitment: "confirmed" });
    config = await fetchConfig(program, configKey);
    console.log("config initialized");
  }
  if (!config) throw new Error("config missing after init");

  // ---------------- agent ----------------
  const agentPublicUrl = env("AGENT_PUBLIC_URL", "http://127.0.0.1:4021").replace(/\/+$/, "");
  const price = new BN(env("AGENT_PRICE", "10000")); // 0.01 USDC

  let previous: Deployment | null = null;
  if (fs.existsSync(DEPLOYMENT_FILE)) {
    previous = JSON.parse(fs.readFileSync(DEPLOYMENT_FILE, "utf8")) as Deployment;
    if (previous.programId !== programId.toBase58() || previous.network !== network) previous = null;
  }

  let agentId: bigint;
  let agentKey: PublicKey;
  const existing = previous ? await fetchAgent(program, new PublicKey(previous.agentPda)) : null;
  if (previous && existing && existing.owner.equals(owner.publicKey)) {
    agentId = BigInt(previous.agentId);
    agentKey = new PublicKey(previous.agentPda);
    console.log(`agent #${agentId} exists, reusing`);
  } else {
    agentId = BigInt(config.nextAgentId.toString());
    agentKey = agentPda(agentId, programId);
    const metadata = {
      type: "relay402-agent-registration-v1",
      name: "Relay402 Demo Agent",
      description: "Answers a prompt. Paid per request with x402 on Solana.",
      network,
      programId: programId.toBase58(),
      agentId: agentId.toString(),
      endpoints: [{ type: "x402", method: "POST", url: `${agentPublicUrl}/api/run` }],
    };
    const metadataBytes = Buffer.from(JSON.stringify(metadata, null, 2) + "\n", "utf8");
    fs.writeFileSync(path.join(ROOT_DIR, "agent-metadata.json"), metadataBytes);

    const ownerProgram = getProgram(connection, owner);
    const payout = await getOrCreateAssociatedTokenAccount(connection, owner, mint, owner.publicKey);
    await ownerProgram.methods
      .registerAgent({
        endpoint: agentPublicUrl,
        metadataUri: `${agentPublicUrl}/metadata.json`,
        metadataHash: Array.from(Buffer.from(sha256Hex(metadataBytes), "hex")),
        price,
        operator: operator.publicKey,
      })
      .accountsStrict({
        owner: owner.publicKey,
        config: configKey,
        agent: agentKey,
        payout: payout.address,
        systemProgram: SystemProgram.programId,
      })
      .rpc({ commitment: "confirmed" });
    console.log(`registered agent #${agentId} at ${agentKey.toBase58()}`);
  }

  // ---------------- client tokens ----------------
  const clientAta = await getOrCreateAssociatedTokenAccount(connection, admin, mint, client.publicKey);
  const mintInfo = await connection.getParsedAccountInfo(mint);
  const mintAuthority = (mintInfo.value?.data as { parsed?: { info?: { mintAuthority?: string } } })
    ?.parsed?.info?.mintAuthority;
  if (createdTestMint || mintAuthority === admin.publicKey.toBase58()) {
    const bal = (await getAccount(connection, clientAta.address)).amount;
    if (bal < 10n * 10n ** BigInt(USDC_DECIMALS)) {
      await mintTo(connection, admin, mint, clientAta.address, admin, 100n * 10n ** BigInt(USDC_DECIMALS));
      console.log("minted 100 test USDC to the client");
    }
  } else {
    console.log(`fund the client with USDC: ${clientAta.address.toBase58()} (owner ${client.publicKey.toBase58()})`);
    console.log("devnet USDC faucet: https://faucet.circle.com");
  }

  const deployment: Deployment = {
    network,
    rpcUrl,
    programId: programId.toBase58(),
    mint: mint.toBase58(),
    treasury: config.treasury.toBase58(),
    agentId: agentId.toString(),
    agentPda: agentKey.toBase58(),
    agentPublicUrl,
  };
  writeDeployment(deployment);
  console.log(`wrote ${DEPLOYMENT_FILE}`);
  console.log(JSON.stringify(deployment, null, 2));
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
