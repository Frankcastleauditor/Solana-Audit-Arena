import { BN, Program } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import crypto from "crypto";
import { fetchAgent, fetchConfig } from "../shared/accounts";
import { agentPda, configPda, receiptPda } from "../shared/pdas";
import type { Relay402 } from "../shared/program";
import {
  PAYMENT_HEADER,
  PAYMENT_RESPONSE_HEADER,
  PaymentPayload,
  PaymentRequirements,
  SCHEME,
  SettleResponse,
  X402_VERSION,
  encodeJsonBase64,
  parsePaymentRequirements,
  parseSettleResponse,
  sha256Hex,
  signAuth,
} from "../shared/x402";

const MIN_WINDOW = 60;
const MAX_WINDOW = 86_400;
/** Extra seconds on top of maxTimeoutSeconds so verify does not reject us. */
const WINDOW_PADDING = 60;

export interface PaidCallOptions {
  program: Program<Relay402>; // provider wallet = client
  client: Keypair;
  network: string;
  url: string;
  body: string;
  /** Hard cap the client is willing to pay, in base units. */
  maxAmount: bigint;
}

export interface PaidCallResult {
  status: number;
  body: unknown;
  paymentResponse?: SettleResponse;
  requirements?: PaymentRequirements;
  receipt?: PublicKey;
  paymentHeader?: string;
}

/**
 * Checks that the 402 quote points at this program, this network, the
 * protocol mint, an existing active agent, and a resource under the agent's
 * registered endpoint. The server is untrusted until these hold.
 */
export async function checkRequirements(
  program: Program<Relay402>,
  network: string,
  url: string,
  reqs: PaymentRequirements,
  maxAmount: bigint,
): Promise<{ agentKey: PublicKey; mint: PublicKey; vault: PublicKey }> {
  if (reqs.scheme !== SCHEME) throw new Error("unsupported scheme");
  if (reqs.network !== network) throw new Error("network mismatch");
  if (reqs.extra.programId !== program.programId.toBase58()) throw new Error("program mismatch");
  if (reqs.resource !== url) throw new Error("resource does not match the requested URL");

  const config = await fetchConfig(program, configPda(program.programId));
  if (!config) throw new Error("protocol not initialized");
  if (reqs.asset !== config.mint.toBase58()) throw new Error("asset is not the protocol mint");

  const agentKey = agentPda(BigInt(reqs.extra.agentId), program.programId);
  if (reqs.payTo !== agentKey.toBase58()) throw new Error("payTo is not the agent PDA");
  const agent = await fetchAgent(program, agentKey);
  if (!agent || !agent.active) throw new Error("agent not found or inactive");
  const endpoint = agent.endpoint.replace(/\/+$/, "");
  if (url !== endpoint && !url.startsWith(`${endpoint}/`)) {
    throw new Error(`URL is not under the agent's registered endpoint (${agent.endpoint})`);
  }

  const price = BigInt(reqs.maxAmountRequired);
  if (price !== BigInt(agent.price.toString())) throw new Error("quoted price differs from on-chain price");
  if (price > maxAmount) throw new Error(`price ${price} is above your max ${maxAmount}`);
  return { agentKey, mint: config.mint, vault: config.vault };
}

export async function createPayment(
  program: Program<Relay402>,
  client: Keypair,
  reqs: PaymentRequirements,
  checked: { agentKey: PublicKey; mint: PublicKey; vault: PublicKey },
): Promise<{ receipt: PublicKey; nonce: bigint; signature: string }> {
  const nonce = crypto.randomBytes(8).readBigUInt64LE();
  const receipt = receiptPda(checked.agentKey, client.publicKey, nonce, program.programId);
  const window = Math.min(MAX_WINDOW, Math.max(MIN_WINDOW, reqs.maxTimeoutSeconds + WINDOW_PADDING));

  const signature = await program.methods
    .createPayment(new BN(nonce.toString()), new BN(window), new BN(reqs.maxAmountRequired))
    .accountsStrict({
      client: client.publicKey,
      config: configPda(program.programId),
      agent: checked.agentKey,
      receipt,
      mint: checked.mint,
      clientToken: getAssociatedTokenAddressSync(checked.mint, client.publicKey),
      vault: checked.vault,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .signers([client])
    .rpc({ commitment: "confirmed" });
  return { receipt, nonce, signature };
}

export function buildPaymentHeader(opts: {
  client: Keypair;
  network: string;
  programId: PublicKey;
  receipt: PublicKey;
  method: string;
  resource: string;
  body: string;
  issuedAt?: number;
}): string {
  const issuedAt = opts.issuedAt ?? Math.floor(Date.now() / 1000);
  const signature = signAuth(
    {
      network: opts.network,
      programId: opts.programId.toBase58(),
      receipt: opts.receipt.toBase58(),
      method: opts.method.toUpperCase(),
      resource: opts.resource,
      bodySha256: sha256Hex(Buffer.from(opts.body, "utf8")),
      issuedAt,
    },
    opts.client.secretKey,
  );
  const payload: PaymentPayload = {
    x402Version: X402_VERSION,
    scheme: SCHEME,
    network: opts.network,
    payload: {
      receipt: opts.receipt.toBase58(),
      client: opts.client.publicKey.toBase58(),
      issuedAt,
      signature,
    },
  };
  return encodeJsonBase64(payload);
}

export async function postWithPayment(
  url: string,
  body: string,
  paymentHeader?: string,
): Promise<{ status: number; body: unknown; paymentResponse?: SettleResponse; raw: globalThis.Response }> {
  const headers: Record<string, string> = { "content-type": "application/json" };
  if (paymentHeader) headers[PAYMENT_HEADER] = paymentHeader;
  const res = await fetch(url, { method: "POST", headers, body });
  const text = await res.text();
  let parsed: unknown = text;
  try {
    parsed = JSON.parse(text);
  } catch {
    /* keep text */
  }
  let paymentResponse: SettleResponse | undefined;
  const h = res.headers.get(PAYMENT_RESPONSE_HEADER);
  if (h) {
    try {
      paymentResponse = parseSettleResponse(JSON.parse(Buffer.from(h, "base64").toString("utf8")));
    } catch {
      /* ignore malformed header */
    }
  }
  return { status: res.status, body: parsed, paymentResponse, raw: res };
}

/** Full x402 round trip: 402 -> pay on-chain -> signed retry. */
export async function paidCall(opts: PaidCallOptions): Promise<PaidCallResult> {
  const first = await postWithPayment(opts.url, opts.body);
  if (first.status !== 402) return { status: first.status, body: first.body };

  const accepts = (first.body as { accepts?: unknown[] }).accepts;
  if (!Array.isArray(accepts)) throw new Error("402 response has no accepts list");
  const reqs = accepts
    .map((a) => {
      try {
        return parsePaymentRequirements(a);
      } catch {
        return null;
      }
    })
    .find((r): r is PaymentRequirements => r !== null && r.scheme === SCHEME);
  if (!reqs) throw new Error("server does not accept relay402-escrow");

  const checked = await checkRequirements(opts.program, opts.network, opts.url, reqs, opts.maxAmount);
  const { receipt } = await createPayment(opts.program, opts.client, reqs, checked);

  const paymentHeader = buildPaymentHeader({
    client: opts.client,
    network: opts.network,
    programId: opts.program.programId,
    receipt,
    method: "POST",
    resource: reqs.resource,
    body: opts.body,
  });
  const second = await postWithPayment(opts.url, opts.body, paymentHeader);
  return {
    status: second.status,
    body: second.body,
    paymentResponse: second.paymentResponse,
    requirements: reqs,
    receipt,
    paymentHeader,
  };
}
