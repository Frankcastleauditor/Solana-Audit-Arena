/**
 * Relay402 facilitator.
 *
 * /verify  Reads the receipt on-chain and checks it pays for the given
 *          requirements. Read-only, no signing.
 * /settle  Receives a settle_payment transaction that the agent operator
 *          already signed, checks it instruction by instruction against what
 *          the facilitator derives from chain state, co-signs as fee payer,
 *          simulates, sends, and confirms.
 *
 * The facilitator holds one key: the fee payer. It has no on-chain role, so
 * a compromised facilitator cannot move escrowed funds. The checks below
 * exist to protect the fee payer's SOL and to make sure only real, pending,
 * correctly-priced receipts are settled.
 */
import { Program } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  Connection,
  Keypair,
  PublicKey,
  TransactionInstruction,
  VersionedTransaction,
} from "@solana/web3.js";
import nacl from "tweetnacl";
import {
  AgentAccount,
  ConfigAccount,
  ReceiptAccount,
  fetchAgent,
  fetchConfig,
  fetchReceipt,
  isPending,
  isSettled,
} from "../shared/accounts";
import { KeyedLock } from "../shared/locks";
import { agentPda, configPda, receiptPda } from "../shared/pdas";
import type { Relay402 } from "../shared/program";
import {
  PaymentPayload,
  PaymentRequirements,
  SCHEME,
  SettleResponse,
  VerifyResponse,
  X402_VERSION,
  parsePaymentPayload,
  parsePaymentRequirements,
} from "../shared/x402";

/** A serialized legacy transaction is at most 1232 bytes (1644 in base64). */
const MAX_TX_BASE64_LEN = 1700;

export interface FacilitatorOptions {
  program: Program<Relay402>;
  feePayer: Keypair;
  network: string;
  /** Minimum seconds a receipt must still be valid when /settle is called. */
  settleMarginSecs?: number;
  confirmTimeoutMs?: number;
  now?: () => number;
}

interface Checked {
  config: ConfigAccount;
  agentKey: PublicKey;
  agent: AgentAccount;
  receiptKey: PublicKey;
  receipt: ReceiptAccount;
}

type CheckResult = { ok: true; value: Checked } | { ok: false; reason: string };

export class Facilitator {
  readonly program: Program<Relay402>;
  readonly connection: Connection;
  readonly feePayer: Keypair;
  readonly network: string;
  private readonly settleMarginSecs: number;
  private readonly confirmTimeoutMs: number;
  private readonly now: () => number;
  private readonly locks = new KeyedLock();

  constructor(opts: FacilitatorOptions) {
    this.program = opts.program;
    this.connection = opts.program.provider.connection;
    this.feePayer = opts.feePayer;
    this.network = opts.network;
    this.settleMarginSecs = opts.settleMarginSecs ?? 10;
    this.confirmTimeoutMs = opts.confirmTimeoutMs ?? 60_000;
    this.now = opts.now ?? (() => Math.floor(Date.now() / 1000));
  }

  supported() {
    return {
      kinds: [
        {
          x402Version: X402_VERSION,
          scheme: SCHEME,
          network: this.network,
          extra: {
            feePayer: this.feePayer.publicKey.toBase58(),
            programId: this.program.programId.toBase58(),
          },
        },
      ],
    };
  }

  async verify(body: unknown): Promise<VerifyResponse> {
    let payload: PaymentPayload;
    let requirements: PaymentRequirements;
    try {
      ({ payload, requirements } = parseRequest(body));
    } catch (e) {
      return { isValid: false, invalidReason: `invalid_request: ${(e as Error).message}` };
    }
    // The receipt must stay valid long enough for the agent to do the work
    // and settle.
    let res: CheckResult;
    try {
      res = await this.check(payload, requirements, requirements.maxTimeoutSeconds);
    } catch (e) {
      return { isValid: false, invalidReason: `invalid_account: ${(e as Error).message}` };
    }
    if (!res.ok) return { isValid: false, invalidReason: res.reason, payer: payload.payload.client };
    return { isValid: true, payer: payload.payload.client };
  }

  async settle(body: unknown): Promise<SettleResponse> {
    const fail = (reason: string, payer?: string): SettleResponse => ({
      success: false,
      errorReason: reason,
      network: this.network,
      payer,
    });

    let payload: PaymentPayload;
    let requirements: PaymentRequirements;
    let txBase64: string;
    try {
      ({ payload, requirements } = parseRequest(body));
      const t = (body as { transaction?: unknown }).transaction;
      if (typeof t !== "string" || t.length === 0 || t.length > MAX_TX_BASE64_LEN) {
        throw new Error("missing or oversized transaction");
      }
      txBase64 = t;
    } catch (e) {
      return fail(`invalid_request: ${(e as Error).message}`);
    }
    const payer = payload.payload.client;

    // One settlement attempt per receipt at a time. Without this, parallel
    // requests would each pay a transaction fee for the same receipt.
    const lockKey = payload.payload.receipt;
    if (!this.locks.tryAcquire(lockKey)) return fail("settlement_in_progress", payer);

    try {
      const res = await this.check(payload, requirements, this.settleMarginSecs);
      if (!res.ok) return fail(res.reason, payer);
      const c = res.value;

      let vtx: VersionedTransaction;
      try {
        vtx = VersionedTransaction.deserialize(Buffer.from(txBase64, "base64"));
      } catch {
        return fail("invalid_transaction_encoding", payer);
      }

      const expected = await this.expectedSettleInstruction(c);
      const txError = this.validateSettleTransaction(vtx, expected, c.agent.operator);
      if (txError) return fail(txError, payer);

      vtx.sign([this.feePayer]);

      // Only transactions that succeed in simulation are sent, so a caller
      // cannot make the fee payer pay for failing transactions.
      const sim = await this.connection.simulateTransaction(vtx, {
        sigVerify: true,
        commitment: "confirmed",
      });
      if (sim.value.err) {
        return fail(`simulation_failed: ${JSON.stringify(sim.value.err)}`, payer);
      }

      const signature = await this.connection.sendRawTransaction(vtx.serialize(), {
        skipPreflight: true,
        maxRetries: 5,
      });
      await this.waitForConfirmation(signature, vtx.message.recentBlockhash);

      // Final truth is the chain, not the RPC status of our transaction.
      const after = await fetchReceipt(this.program, c.receiptKey, "confirmed");
      if (after === null || !isSettled(after)) {
        return { ...fail("settlement_not_confirmed", payer), transaction: signature };
      }
      return { success: true, transaction: signature, network: this.network, payer };
    } catch (e) {
      return fail(`internal_error: ${(e as Error).message}`, payer);
    } finally {
      this.locks.release(lockKey);
    }
  }

  // -------------------------------------------------------------------

  private async check(
    payload: PaymentPayload,
    req: PaymentRequirements,
    minRemainingSecs: number,
  ): Promise<CheckResult> {
    const no = (reason: string): CheckResult => ({ ok: false, reason });

    if (payload.scheme !== SCHEME || req.scheme !== SCHEME) return no("unsupported_scheme");
    if (payload.network !== this.network || req.network !== this.network) {
      return no("invalid_network");
    }
    if (req.extra.programId !== this.program.programId.toBase58()) return no("invalid_program");
    if (req.extra.feePayer !== this.feePayer.publicKey.toBase58()) return no("invalid_fee_payer");

    const config = await fetchConfig(this.program, configPda(this.program.programId));
    if (config === null) return no("protocol_not_initialized");
    if (req.asset !== config.mint.toBase58()) return no("invalid_asset");

    const agentId = BigInt(req.extra.agentId);
    const agentKey = agentPda(agentId, this.program.programId);
    if (req.payTo !== agentKey.toBase58()) return no("invalid_pay_to");
    const agent = await fetchAgent(this.program, agentKey);
    if (agent === null) return no("agent_not_found");

    const receiptKey = new PublicKey(payload.payload.receipt);
    const receipt = await fetchReceipt(this.program, receiptKey);
    if (receipt === null) return no("receipt_not_found");
    if (!receipt.agent.equals(agentKey)) return no("receipt_agent_mismatch");
    if (receipt.client.toBase58() !== payload.payload.client) return no("receipt_client_mismatch");
    const canonical = receiptPda(
      agentKey,
      receipt.client,
      BigInt(receipt.nonce.toString()),
      this.program.programId,
    );
    if (!canonical.equals(receiptKey)) return no("receipt_address_mismatch");
    if (!isPending(receipt)) return no("receipt_not_pending");
    if (BigInt(receipt.amount.toString()) < BigInt(req.maxAmountRequired)) {
      return no("insufficient_amount");
    }
    const remaining = BigInt(receipt.expiresAt.toString()) - BigInt(this.now());
    if (remaining < BigInt(minRemainingSecs)) return no("receipt_expires_too_soon");

    return { ok: true, value: { config, agentKey, agent, receiptKey, receipt } };
  }

  private expectedSettleInstruction(c: Checked): Promise<TransactionInstruction> {
    return this.program.methods
      .settlePayment()
      .accountsStrict({
        operator: c.agent.operator,
        config: configPda(this.program.programId),
        payout: c.agent.payout,
        agent: c.agentKey,
        receipt: c.receiptKey,
        mint: c.config.mint,
        vault: c.config.vault,
        treasury: c.config.treasury,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
  }

  /**
   * The transaction must be exactly: legacy message, fee payer = us, two
   * signers (us, operator), one instruction equal to `expected`, no extra
   * account keys, and a valid operator signature over the message.
   */
  private validateSettleTransaction(
    vtx: VersionedTransaction,
    expected: TransactionInstruction,
    operator: PublicKey,
  ): string | null {
    if (vtx.version !== "legacy") return "unsupported_transaction_version";
    const msg = vtx.message;
    const keys = msg.staticAccountKeys;

    if (operator.equals(this.feePayer.publicKey)) return "operator_is_fee_payer";
    if (keys.length === 0 || !keys[0].equals(this.feePayer.publicKey)) return "invalid_fee_payer";
    if (msg.header.numRequiredSignatures !== 2) return "invalid_signer_count";
    if (!keys[1].equals(operator)) return "invalid_operator";
    if (msg.compiledInstructions.length !== 1) return "unexpected_instructions";

    const ix = msg.compiledInstructions[0];
    if (!keys[ix.programIdIndex].equals(expected.programId)) return "invalid_program";
    if (!Buffer.from(ix.data).equals(expected.data)) return "invalid_instruction_data";
    if (ix.accountKeyIndexes.length !== expected.keys.length) return "invalid_instruction_accounts";
    // The fee payer must not be passed into the instruction in any role.
    if (ix.accountKeyIndexes.includes(0)) return "fee_payer_in_instruction";

    for (let i = 0; i < expected.keys.length; i++) {
      const idx = ix.accountKeyIndexes[i];
      const want = expected.keys[i];
      if (
        !keys[idx].equals(want.pubkey) ||
        msg.isAccountSigner(idx) !== want.isSigner ||
        msg.isAccountWritable(idx) !== want.isWritable
      ) {
        return "invalid_instruction_accounts";
      }
    }

    const used = new Set<number>([0, ix.programIdIndex, ...ix.accountKeyIndexes]);
    if (used.size !== keys.length) return "unexpected_account_keys";

    const opSig = vtx.signatures[1];
    if (!opSig || !nacl.sign.detached.verify(msg.serialize(), opSig, operator.toBytes())) {
      return "invalid_operator_signature";
    }
    return null;
  }

  private async waitForConfirmation(signature: string, blockhash: string): Promise<void> {
    const deadline = Date.now() + this.confirmTimeoutMs;
    while (Date.now() < deadline) {
      const { value } = await this.connection.getSignatureStatuses([signature]);
      const status = value[0];
      if (status) {
        if (status.err) return; // caller re-reads the receipt and reports failure
        if (status.confirmationStatus === "confirmed" || status.confirmationStatus === "finalized") {
          return;
        }
      } else {
        const valid = await this.connection.isBlockhashValid(blockhash, { commitment: "confirmed" });
        if (!valid.value) return; // expired without landing
      }
      await new Promise((r) => setTimeout(r, 400));
    }
  }
}

function parseRequest(body: unknown): { payload: PaymentPayload; requirements: PaymentRequirements } {
  if (typeof body !== "object" || body === null) throw new Error("body must be a JSON object");
  const b = body as Record<string, unknown>;
  if (b.x402Version !== X402_VERSION) throw new Error("unsupported x402Version");
  return {
    payload: parsePaymentPayload(b.paymentPayload),
    requirements: parsePaymentRequirements(b.paymentRequirements),
  };
}

