/**
 * x402 wire format used by Relay402 (x402 v1 message shapes, custom scheme).
 *
 * Scheme "relay402-escrow":
 *   1. Server answers 402 with PaymentRequirements (price read from chain).
 *   2. Client locks the price on-chain with `create_payment` (Receipt PDA).
 *   3. Client retries with X-PAYMENT = base64(JSON(PaymentPayload)). The
 *      payload names the receipt and carries an ed25519 signature by the
 *      receipt's client over the exact request (method, resource, body hash).
 *   4. Server checks the signature, asks the facilitator to /verify the
 *      receipt, runs the task, has the facilitator /settle, and only then
 *      returns the result with X-PAYMENT-RESPONSE.
 */
import crypto from "crypto";
import bs58 from "bs58";
import nacl from "tweetnacl";
import { PublicKey } from "@solana/web3.js";

export const X402_VERSION = 1;
export const SCHEME = "relay402-escrow";
export const PAYMENT_HEADER = "x-payment";
export const PAYMENT_RESPONSE_HEADER = "X-PAYMENT-RESPONSE";

/** Max accepted length of an encoded X-PAYMENT header. */
export const MAX_PAYMENT_HEADER_LEN = 2048;
/** Signed requests are accepted for this long after issuedAt. */
export const AUTH_MAX_AGE_SECS = 300;
/** Tolerated client clock skew into the future. */
export const AUTH_MAX_FUTURE_SECS = 60;

export interface PaymentRequirements {
  scheme: string;
  network: string;
  /** Price in base units of `asset`, decimal string. */
  maxAmountRequired: string;
  resource: string;
  description: string;
  mimeType: string;
  /** Agent PDA. Funds go to the agent's payout account on settlement. */
  payTo: string;
  /** The receipt must stay valid (not expired) for at least this long at verify time. */
  maxTimeoutSeconds: number;
  /** Payment mint. */
  asset: string;
  extra: {
    programId: string;
    agentId: string;
    /** Facilitator fee payer for the settlement transaction. */
    feePayer: string;
  };
}

export interface PaymentRequiredBody {
  x402Version: number;
  error: string;
  accepts: PaymentRequirements[];
}

export interface PaymentPayload {
  x402Version: number;
  scheme: string;
  network: string;
  payload: {
    receipt: string;
    client: string;
    issuedAt: number;
    /** base58 ed25519 signature over buildAuthMessage(...) */
    signature: string;
  };
}

export interface VerifyRequest {
  x402Version: number;
  paymentPayload: PaymentPayload;
  paymentRequirements: PaymentRequirements;
}

export interface VerifyResponse {
  isValid: boolean;
  invalidReason?: string;
  payer?: string;
}

export interface SettleRequest extends VerifyRequest {
  /** base64 legacy transaction: fee payer = facilitator, signed by the agent operator. */
  transaction: string;
}

export interface SettleResponse {
  success: boolean;
  errorReason?: string;
  transaction?: string;
  network: string;
  payer?: string;
}

export interface AuthFields {
  network: string;
  programId: string;
  receipt: string;
  method: string;
  resource: string;
  bodySha256: string;
  issuedAt: number;
}

export function sha256Hex(data: Uint8Array | string): string {
  return crypto.createHash("sha256").update(data).digest("hex");
}

/**
 * Domain-separated message the client signs. Every field is on its own line
 * and none of them can contain a newline (checked in `assertAuthFields`), so
 * two different requests cannot produce the same message.
 */
export function buildAuthMessage(f: AuthFields): Uint8Array {
  assertAuthFields(f);
  const text = [
    "relay402-auth:v1",
    `network:${f.network}`,
    `program:${f.programId}`,
    `receipt:${f.receipt}`,
    `method:${f.method}`,
    `resource:${f.resource}`,
    `body-sha256:${f.bodySha256}`,
    `issued-at:${f.issuedAt}`,
  ].join("\n");
  return new TextEncoder().encode(text);
}

function assertAuthFields(f: AuthFields): void {
  for (const [k, v] of Object.entries(f)) {
    if (typeof v === "string" && /[\r\n]/.test(v)) {
      throw new Error(`auth field ${k} contains a newline`);
    }
  }
  if (!/^[0-9a-f]{64}$/.test(f.bodySha256)) throw new Error("bodySha256 must be 64 hex chars");
  if (!Number.isSafeInteger(f.issuedAt) || f.issuedAt <= 0) throw new Error("bad issuedAt");
}

export function signAuth(f: AuthFields, secretKey: Uint8Array): string {
  return bs58.encode(nacl.sign.detached(buildAuthMessage(f), secretKey));
}

export function verifyAuth(f: AuthFields, signature: string, signer: PublicKey): boolean {
  let sig: Uint8Array;
  try {
    sig = bs58.decode(signature);
  } catch {
    return false;
  }
  if (sig.length !== nacl.sign.signatureLength) return false;
  return nacl.sign.detached.verify(buildAuthMessage(f), sig, signer.toBytes());
}

export function encodeJsonBase64(value: unknown): string {
  return Buffer.from(JSON.stringify(value), "utf8").toString("base64");
}

// ---------------------------------------------------------------------
// Strict parsing. Everything coming over HTTP is untrusted.
// ---------------------------------------------------------------------

function isObject(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function isPubkeyString(v: unknown): v is string {
  if (typeof v !== "string" || v.length < 32 || v.length > 44) return false;
  try {
    new PublicKey(v);
    return true;
  } catch {
    return false;
  }
}

const U64_MAX = (1n << 64n) - 1n;

export function isU64String(v: unknown): v is string {
  return typeof v === "string" && /^(0|[1-9]\d{0,19})$/.test(v) && BigInt(v) <= U64_MAX;
}

export function parsePaymentPayload(v: unknown): PaymentPayload {
  if (!isObject(v) || !isObject(v.payload)) throw new Error("malformed payment payload");
  const p = v.payload;
  if (v.x402Version !== X402_VERSION) throw new Error("unsupported x402Version");
  if (typeof v.scheme !== "string" || typeof v.network !== "string") {
    throw new Error("malformed payment payload");
  }
  if (!isPubkeyString(p.receipt) || !isPubkeyString(p.client)) {
    throw new Error("malformed receipt or client");
  }
  if (typeof p.issuedAt !== "number" || !Number.isSafeInteger(p.issuedAt) || p.issuedAt <= 0) {
    throw new Error("malformed issuedAt");
  }
  if (typeof p.signature !== "string" || p.signature.length > 100) {
    throw new Error("malformed signature");
  }
  return {
    x402Version: X402_VERSION,
    scheme: v.scheme,
    network: v.network,
    payload: {
      receipt: p.receipt,
      client: p.client,
      issuedAt: p.issuedAt,
      signature: p.signature,
    },
  };
}

export function decodePaymentHeader(header: string): PaymentPayload {
  if (header.length === 0 || header.length > MAX_PAYMENT_HEADER_LEN) {
    throw new Error("X-PAYMENT header has invalid length");
  }
  if (!/^[A-Za-z0-9+/]+={0,2}$/.test(header)) throw new Error("X-PAYMENT is not base64");
  let json: unknown;
  try {
    json = JSON.parse(Buffer.from(header, "base64").toString("utf8"));
  } catch {
    throw new Error("X-PAYMENT is not valid JSON");
  }
  return parsePaymentPayload(json);
}

export function parsePaymentRequirements(v: unknown): PaymentRequirements {
  if (!isObject(v) || !isObject(v.extra)) throw new Error("malformed payment requirements");
  const e = v.extra;
  const strings = ["scheme", "network", "resource", "description", "mimeType"] as const;
  for (const k of strings) {
    if (typeof v[k] !== "string" || (v[k] as string).length > 512) {
      throw new Error(`malformed requirements.${k}`);
    }
  }
  if (!isU64String(v.maxAmountRequired)) throw new Error("malformed maxAmountRequired");
  if (!isPubkeyString(v.payTo) || !isPubkeyString(v.asset)) throw new Error("malformed payTo/asset");
  if (
    typeof v.maxTimeoutSeconds !== "number" ||
    !Number.isSafeInteger(v.maxTimeoutSeconds) ||
    v.maxTimeoutSeconds <= 0 ||
    v.maxTimeoutSeconds > 86_400
  ) {
    throw new Error("malformed maxTimeoutSeconds");
  }
  if (!isPubkeyString(e.programId) || !isPubkeyString(e.feePayer) || !isU64String(e.agentId)) {
    throw new Error("malformed requirements.extra");
  }
  return {
    scheme: v.scheme as string,
    network: v.network as string,
    maxAmountRequired: v.maxAmountRequired,
    resource: v.resource as string,
    description: v.description as string,
    mimeType: v.mimeType as string,
    payTo: v.payTo,
    maxTimeoutSeconds: v.maxTimeoutSeconds,
    asset: v.asset,
    extra: { programId: e.programId, agentId: e.agentId, feePayer: e.feePayer },
  };
}

export function parseSettleResponse(v: unknown): SettleResponse {
  if (!isObject(v) || typeof v.success !== "boolean" || typeof v.network !== "string") {
    throw new Error("malformed settle response");
  }
  return {
    success: v.success,
    network: v.network,
    errorReason: typeof v.errorReason === "string" ? v.errorReason : undefined,
    transaction: typeof v.transaction === "string" ? v.transaction : undefined,
    payer: typeof v.payer === "string" ? v.payer : undefined,
  };
}

export function parseVerifyResponse(v: unknown): VerifyResponse {
  if (!isObject(v) || typeof v.isValid !== "boolean") throw new Error("malformed verify response");
  return {
    isValid: v.isValid,
    invalidReason: typeof v.invalidReason === "string" ? v.invalidReason : undefined,
    payer: typeof v.payer === "string" ? v.payer : undefined,
  };
}

/** Deep equality for requirements (used to make sure nothing was altered). */
export function sameRequirements(a: PaymentRequirements, b: PaymentRequirements): boolean {
  return JSON.stringify(canonical(a)) === JSON.stringify(canonical(b));
}

function canonical(r: PaymentRequirements) {
  return {
    scheme: r.scheme,
    network: r.network,
    maxAmountRequired: r.maxAmountRequired,
    resource: r.resource,
    description: r.description,
    mimeType: r.mimeType,
    payTo: r.payTo,
    maxTimeoutSeconds: r.maxTimeoutSeconds,
    asset: r.asset,
    extra: {
      programId: r.extra.programId,
      agentId: r.extra.agentId,
      feePayer: r.extra.feePayer,
    },
  };
}
