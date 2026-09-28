import "dotenv/config";
import fs from "fs";
import os from "os";
import path from "path";
import { Keypair } from "@solana/web3.js";

export const ROOT_DIR = path.resolve(__dirname, "..", "..");
export const KEYS_DIR = path.join(ROOT_DIR, ".keys");
export const DEPLOYMENT_FILE = path.join(ROOT_DIR, "deployment.json");

export type Network = "solana-localnet" | "solana-devnet" | "solana";

const NETWORKS: readonly Network[] = ["solana-localnet", "solana-devnet", "solana"];

export function env(name: string, fallback?: string): string {
  const value = process.env[name] ?? fallback;
  if (value === undefined || value === "") {
    throw new Error(`Missing required environment variable ${name}`);
  }
  return value;
}

export function envInt(name: string, fallback: number): number {
  const raw = process.env[name];
  if (raw === undefined || raw === "") return fallback;
  if (!/^\d+$/.test(raw)) throw new Error(`${name} must be a non-negative integer`);
  return Number(raw);
}

export function networkFromEnv(): Network {
  const value = env("NETWORK", "solana-localnet");
  if (!(NETWORKS as readonly string[]).includes(value)) {
    throw new Error(`NETWORK must be one of ${NETWORKS.join(", ")}`);
  }
  return value as Network;
}

export function rpcUrlFromEnv(): string {
  return env("RPC_URL", "http://127.0.0.1:8899");
}

export function expandHome(p: string): string {
  return p.startsWith("~") ? path.join(os.homedir(), p.slice(1)) : p;
}

export function loadKeypair(file: string): Keypair {
  const raw = JSON.parse(fs.readFileSync(expandHome(file), "utf8"));
  if (!Array.isArray(raw) || raw.length !== 64) {
    throw new Error(`${file} is not a Solana keypair file`);
  }
  return Keypair.fromSecretKey(Uint8Array.from(raw));
}

export function saveKeypair(file: string, kp: Keypair): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(Array.from(kp.secretKey)), { mode: 0o600 });
}

export function loadOrCreateKeypair(file: string): Keypair {
  if (fs.existsSync(file)) return loadKeypair(file);
  const kp = Keypair.generate();
  saveKeypair(file, kp);
  return kp;
}

export interface Deployment {
  network: Network;
  rpcUrl: string;
  programId: string;
  mint: string;
  treasury: string;
  agentId: string;
  agentPda: string;
  agentPublicUrl: string;
}

export function readDeployment(): Deployment {
  if (!fs.existsSync(DEPLOYMENT_FILE)) {
    throw new Error(`${DEPLOYMENT_FILE} not found. Run "npm run setup" first.`);
  }
  return JSON.parse(fs.readFileSync(DEPLOYMENT_FILE, "utf8")) as Deployment;
}

export function writeDeployment(d: Deployment): void {
  fs.writeFileSync(DEPLOYMENT_FILE, JSON.stringify(d, null, 2) + "\n");
}
