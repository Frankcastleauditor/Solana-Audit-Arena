/**
 * x402 paywall for a single paid route.
 *
 * Order of operations (and why):
 *   1. Validate input            -> never charge for a request we would reject
 *   2. Read agent price on-chain -> the quote always matches the chain
 *   3. No X-PAYMENT              -> 402 + requirements
 *   4. Check client signature    -> binds the payment to THIS exact request
 *   5. Lock the receipt          -> no parallel double processing
 *   6. Facilitator /verify       -> receipt is pending, funded, not expiring
 *   7. Run the task
 *   8. Facilitator /settle       -> funds move on-chain
 *   9. Return the result         -> only after settlement is confirmed
 * If anything fails after step 6, nothing is settled and the client can
 * refund after the receipt expires.
 */
import { Program } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  Keypair,
  PublicKey,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { Request, Response } from "express";
import { AgentAccount, fetchAgent, fetchConfig } from "../shared/accounts";
import { postJson } from "../shared/http";
import { KeyedLock } from "../shared/locks";
import { configPda } from "../shared/pdas";
import type { Relay402 } from "../shared/program";
import {
  AUTH_MAX_AGE_SECS,
  AUTH_MAX_FUTURE_SECS,
  PAYMENT_HEADER,
  PAYMENT_RESPONSE_HEADER,
  PaymentPayload,
  PaymentRequiredBody,
  PaymentRequirements,
  SCHEME,
  X402_VERSION,
  decodePaymentHeader,
  encodeJsonBase64,
  parseSettleResponse,
  parseVerifyResponse,
  sha256Hex,
  verifyAuth,
} from "../shared/x402";

export interface PaywallOptions {
  program: Program<Relay402>;
  network: string;
  agentId: bigint;
  agentKey: PublicKey;
  operator: Keypair;
  mint: PublicKey;
  facilitatorUrl: string;
  facilitatorFeePayer: PublicKey;
  /** Public origin of this server, e.g. https://agent.example.com (no trailing slash). */
  publicBaseUrl: string;
  description: string;
  maxTimeoutSeconds: number;
  taskTimeoutMs: number;
  facilitatorTimeoutMs: number;
  now?: () => number;
}

export type PaidHandler<I, O> = {
  parse: (raw: Buffer) => I;
  run: (input: I) => Promise<O>;
};

function paymentRequired(res: Response, error: string, requirements?: PaymentRequirements) {
  const body: PaymentRequiredBody = {
    x402Version: X402_VERSION,
    error,
    accepts: requirements ? [requirements] : [],
  };
  res.status(402).json(body);
}

function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const t = setTimeout(() => reject(new Error("task timed out")), ms);
    p.then(
      (v) => {
        clearTimeout(t);
        resolve(v);
      },
      (e) => {
        clearTimeout(t);
        reject(e);
      },
    );
  });
}

export function createPaywall<I, O>(opts: PaywallOptions, handler: PaidHandler<I, O>) {
  const locks = new KeyedLock();
  const now = opts.now ?? (() => Math.floor(Date.now() / 1000));
  const programId = opts.program.programId;

  function buildRequirements(agent: AgentAccount, resource: string): PaymentRequirements {
    return {
      scheme: SCHEME,
      network: opts.network,
      maxAmountRequired: agent.price.toString(),
      resource,
      description: opts.description,
      mimeType: "application/json",
      payTo: opts.agentKey.toBase58(),
      maxTimeoutSeconds: opts.maxTimeoutSeconds,
      asset: opts.mint.toBase58(),
      extra: {
        programId: programId.toBase58(),
        agentId: opts.agentId.toString(),
        feePayer: opts.facilitatorFeePayer.toBase58(),
      },
    };
  }

  async function buildSettleTransaction(receipt: PublicKey): Promise<string> {
    // Fresh reads: operator, payout and treasury can be rotated at any time.
    const config = await fetchConfig(opts.program, configPda(programId));
    const agent = await fetchAgent(opts.program, opts.agentKey);
    if (!config || !agent) throw new Error("protocol state missing");
    if (!agent.operator.equals(opts.operator.publicKey)) {
      throw new Error("this server's key is no longer the agent operator");
    }
    const ix = await opts.program.methods
      .settlePayment()
      .accountsStrict({
        operator: opts.operator.publicKey,
        config: configPda(programId),
        payout: agent.payout,
        agent: opts.agentKey,
        receipt,
        mint: config.mint,
        vault: config.vault,
        treasury: config.treasury,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    const { blockhash } = await opts.program.provider.connection.getLatestBlockhash("confirmed");
    const message = new TransactionMessage({
      payerKey: opts.facilitatorFeePayer,
      recentBlockhash: blockhash,
      instructions: [ix],
    }).compileToLegacyMessage();
    const vtx = new VersionedTransaction(message);
    vtx.sign([opts.operator]);
    return Buffer.from(vtx.serialize()).toString("base64");
  }

  return async function paywall(req: Request, res: Response): Promise<void> {
    const rawBody: Buffer = Buffer.isBuffer(req.body) ? req.body : Buffer.alloc(0);

    // 1. input
    let input: I;
    try {
      input = handler.parse(rawBody);
    } catch (e) {
      res.status(400).json({ error: (e as Error).message });
      return;
    }

    // 2. price and status from chain
    const agent = await fetchAgent(opts.program, opts.agentKey);
    if (agent === null || !agent.active) {
      res.status(503).json({ error: "agent is not active" });
      return;
    }
    // Built from our configured public origin, never from the Host header.
    const resource = `${opts.publicBaseUrl}${req.originalUrl}`;
    const requirements = buildRequirements(agent, resource);

    // 3. payment header
    const header = req.header(PAYMENT_HEADER);
    if (!header) {
      paymentRequired(res, "X-PAYMENT header is required", requirements);
      return;
    }
    let payment: PaymentPayload;
    try {
      payment = decodePaymentHeader(header);
    } catch (e) {
      paymentRequired(res, `invalid_payment: ${(e as Error).message}`, requirements);
      return;
    }
    if (payment.scheme !== SCHEME || payment.network !== opts.network) {
      paymentRequired(res, "invalid_payment: scheme or network mismatch", requirements);
      return;
    }

    // 4. request binding
    const t = now();
    const issuedAt = payment.payload.issuedAt;
    if (issuedAt < t - AUTH_MAX_AGE_SECS || issuedAt > t + AUTH_MAX_FUTURE_SECS) {
      paymentRequired(res, "invalid_payment: stale signature", requirements);
      return;
    }
    const signed = verifyAuth(
      {
        network: opts.network,
        programId: programId.toBase58(),
        receipt: payment.payload.receipt,
        method: req.method.toUpperCase(),
        resource,
        bodySha256: sha256Hex(rawBody),
        issuedAt,
      },
      payment.payload.signature,
      new PublicKey(payment.payload.client),
    );
    if (!signed) {
      paymentRequired(res, "invalid_payment: bad client signature", requirements);
      return;
    }

    // 5. lock
    const lockKey = payment.payload.receipt;
    if (!locks.tryAcquire(lockKey)) {
      res.status(409).json({ error: "payment is already being processed" });
      return;
    }

    try {
      // 6. verify
      const verifyBody = {
        x402Version: X402_VERSION,
        paymentPayload: payment,
        paymentRequirements: requirements,
      };
      let verified;
      try {
        verified = parseVerifyResponse(
          await postJson(`${opts.facilitatorUrl}/verify`, verifyBody, opts.facilitatorTimeoutMs),
        );
      } catch (e) {
        res.status(502).json({ error: `facilitator unavailable: ${(e as Error).message}` });
        return;
      }
      if (!verified.isValid) {
        paymentRequired(res, `invalid_payment: ${verified.invalidReason ?? "rejected"}`, requirements);
        return;
      }

      // 7. work
      let output: O;
      try {
        output = await withTimeout(handler.run(input), opts.taskTimeoutMs);
      } catch (e) {
        console.error("[agent] task failed:", e);
        res.status(500).json({ error: "task failed, payment not settled" });
        return;
      }

      // 8. settle
      let settled;
      try {
        const transaction = await buildSettleTransaction(new PublicKey(payment.payload.receipt));
        settled = parseSettleResponse(
          await postJson(
            `${opts.facilitatorUrl}/settle`,
            { ...verifyBody, transaction },
            opts.facilitatorTimeoutMs,
          ),
        );
      } catch (e) {
        res.status(502).json({ error: `settlement unavailable: ${(e as Error).message}` });
        return;
      }
      if (!settled.success) {
        paymentRequired(res, `settlement_failed: ${settled.errorReason ?? "unknown"}`, requirements);
        return;
      }

      // 9. deliver
      res.setHeader(PAYMENT_RESPONSE_HEADER, encodeJsonBase64(settled));
      res.status(200).json(output);
    } finally {
      locks.release(lockKey);
    }
  };
}
