import { BN } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import {
  Keypair,
  PublicKey,
  SystemProgram,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
} from "@solana/web3.js";
import { expect } from "chai";
import { Server } from "http";
import { AddressInfo } from "net";
import { createAgentApp } from "../offchain/agent-server/server";
import {
  buildPaymentHeader,
  checkRequirements,
  createPayment as clientCreatePayment,
  paidCall,
  postWithPayment,
} from "../offchain/client/lib";
import { Facilitator } from "../offchain/facilitator/facilitator";
import { createFacilitatorApp } from "../offchain/facilitator/server";
import { fetchReceipt, isPending, isSettled } from "../offchain/shared/accounts";
import { getProgram } from "../offchain/shared/program";
import {
  PaymentRequiredBody,
  PaymentRequirements,
  SCHEME,
  X402_VERSION,
  decodePaymentHeader,
  sha256Hex,
} from "../offchain/shared/x402";
import {
  CONFIG,
  VAULT,
  ata,
  connection,
  expectError,
  feedback,
  fetchConfigState,
  fund,
  newUser,
  program,
  registerAgent,
  tokenBalance,
} from "./helpers";

const NETWORK = "solana-localnet";
const PRICE = 10_000n;

function listen(app: { listen: (port: number, host: string, cb: () => void) => Server }): Promise<Server> {
  return new Promise((resolve) => {
    const s: Server = app.listen(0, "127.0.0.1", () => resolve(s));
  });
}
const urlOf = (s: Server) => `http://127.0.0.1:${(s.address() as AddressInfo).port}`;

describe("relay402: end to end (client -> agent server -> facilitator -> chain)", () => {
  let mint: PublicKey;
  let treasury: PublicKey;
  let owner: Keypair;
  let operator: Keypair;
  let client: Keypair;
  let attacker: Keypair;
  let feePayer: Keypair;
  let payout: PublicKey;
  let clientToken: PublicKey;
  let agentId: bigint;
  let agent: PublicKey;
  let facilitator: Facilitator;
  let facServer: Server;
  let agentServer: Server;
  let agentUrl: string;
  let runUrl: string;
  let metadata: Buffer;

  const clientProgram = () => getProgram(connection, client);

  before(async () => {
    const config = await fetchConfigState();
    mint = config.mint;
    treasury = config.treasury;
    [owner, operator, client, attacker, feePayer] = await Promise.all([
      newUser(),
      newUser(1),
      newUser(),
      newUser(),
      newUser(2),
    ]);
    payout = await ata(mint, owner.publicKey);
    clientToken = await fund(mint, client.publicKey, 1_000_000_000n);
    await fund(mint, attacker.publicKey, 1_000_000_000n);

    facilitator = new Facilitator({ program: getProgram(connection, feePayer), feePayer, network: NETWORK });
    facServer = await listen(createFacilitatorApp(facilitator));

    // Reserve a port for the agent so the on-chain endpoint is known up front.
    const probe = await listen(createFacilitatorApp(facilitator));
    const port = (probe.address() as AddressInfo).port;
    await new Promise((r) => probe.close(r));
    agentUrl = `http://127.0.0.1:${port}`;
    runUrl = `${agentUrl}/api/run`;

    metadata = Buffer.from(JSON.stringify({ name: "e2e agent" }));
    const r = await registerAgent({
      owner,
      operator: operator.publicKey,
      payout,
      price: PRICE,
      endpoint: agentUrl,
      metadataUri: `${agentUrl}/metadata.json`,
      metadataHash: Array.from(Buffer.from(sha256Hex(metadata), "hex")),
    });
    agentId = r.id;
    agent = r.agent;

    const app = createAgentApp({
      program: getProgram(connection, operator),
      network: NETWORK,
      agentId,
      agentKey: agent,
      operator,
      mint,
      facilitatorUrl: urlOf(facServer),
      facilitatorFeePayer: feePayer.publicKey,
      publicBaseUrl: agentUrl,
      description: "e2e",
      maxTimeoutSeconds: 120,
      taskTimeoutMs: 10_000,
      facilitatorTimeoutMs: 60_000,
      metadata,
    });
    agentServer = await new Promise<Server>((resolve) => {
      const s: Server = app.listen(port, "127.0.0.1", () => resolve(s));
    });
  });

  after(async () => {
    await new Promise((r) => agentServer.close(r));
    await new Promise((r) => facServer.close(r));
  });

  async function quote(): Promise<PaymentRequirements> {
    const res = await postWithPayment(runUrl, JSON.stringify({ prompt: "hi" }));
    expect(res.status).to.equal(402);
    return (res.body as PaymentRequiredBody).accepts[0];
  }

  async function payOnly(): Promise<{ reqs: PaymentRequirements; receipt: PublicKey }> {
    const reqs = await quote();
    const checked = await checkRequirements(clientProgram(), NETWORK, runUrl, reqs, PRICE);
    const { receipt } = await clientCreatePayment(clientProgram(), client, reqs, checked);
    return { reqs, receipt };
  }

  async function settleTx(opts: {
    receipt: PublicKey;
    payer?: PublicKey;
    signers?: Keypair[];
    operatorKey?: PublicKey;
    extra?: TransactionInstruction[];
  }): Promise<string> {
    const config = await fetchConfigState();
    const ix = await program.methods
      .settlePayment()
      .accountsStrict({
        operator: opts.operatorKey ?? operator.publicKey,
        config: CONFIG,
        payout,
        agent,
        receipt: opts.receipt,
        mint,
        vault: VAULT,
        treasury: config.treasury,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .instruction();
    const { blockhash } = await connection.getLatestBlockhash("confirmed");
    const msg = new TransactionMessage({
      payerKey: opts.payer ?? feePayer.publicKey,
      recentBlockhash: blockhash,
      instructions: [...(opts.extra ?? []), ix],
    }).compileToLegacyMessage();
    const vtx = new VersionedTransaction(msg);
    if ((opts.signers ?? [operator]).length) vtx.sign(opts.signers ?? [operator]);
    return Buffer.from(vtx.serialize()).toString("base64");
  }

  function payloadFor(receipt: PublicKey) {
    return decodePaymentHeader(
      buildPaymentHeader({
        client,
        network: NETWORK,
        programId: program.programId,
        receipt,
        method: "POST",
        resource: runUrl,
        body: "{}",
      }),
    );
  }

  // --------------------------------------------------------------- happy path

  it("serves metadata whose hash matches the chain", async () => {
    const res = await fetch(`${agentUrl}/metadata.json`);
    const body = Buffer.from(await res.arrayBuffer());
    const a = await program.account.agent.fetch(agent);
    expect(sha256Hex(body)).to.equal(Buffer.from(a.metadataHash).toString("hex"));
  });

  it("rejects invalid input before asking for payment", async () => {
    const res = await postWithPayment(runUrl, JSON.stringify({ prompt: "" }));
    expect(res.status).to.equal(400);
  });

  it("answers 402 with requirements that match the chain", async () => {
    const reqs = await quote();
    expect(reqs.scheme).to.equal(SCHEME);
    expect(reqs.network).to.equal(NETWORK);
    expect(reqs.maxAmountRequired).to.equal(PRICE.toString());
    expect(reqs.payTo).to.equal(agent.toBase58());
    expect(reqs.asset).to.equal(mint.toBase58());
    expect(reqs.resource).to.equal(runUrl);
    expect(reqs.extra.agentId).to.equal(agentId.toString());
    expect(reqs.extra.feePayer).to.equal(feePayer.publicKey.toBase58());
  });

  let paidReceipt: PublicKey;
  let paidHeader: string;

  it("full paid call: pay, verify, run, settle, deliver", async () => {
    const payout0 = await tokenBalance(payout);
    const treasury0 = await tokenBalance(treasury);
    const client0 = await tokenBalance(clientToken);
    const fee0 = await connection.getBalance(feePayer.publicKey);

    const result = await paidCall({
      program: clientProgram(),
      client,
      network: NETWORK,
      url: runUrl,
      body: JSON.stringify({ prompt: "hello there agent" }),
      maxAmount: PRICE,
    });
    expect(result.status, JSON.stringify(result.body)).to.equal(200);
    expect(result.body).to.deep.equal({ output: "stub agent: received 3 word(s)", model: "stub" });
    expect(result.paymentResponse?.success).to.equal(true);
    expect(result.paymentResponse?.payer).to.equal(client.publicKey.toBase58());

    expect(await tokenBalance(payout)).to.equal(payout0 + 9_900n);
    expect(await tokenBalance(treasury)).to.equal(treasury0 + 100n);
    expect(await tokenBalance(clientToken)).to.equal(client0 - PRICE);
    // facilitator paid the two signature fees and nothing else
    expect(fee0 - (await connection.getBalance(feePayer.publicKey))).to.equal(10_000);

    const r = await fetchReceipt(program, result.receipt!);
    expect(r && isSettled(r)).to.equal(true);
    paidReceipt = result.receipt!;
    paidHeader = result.paymentHeader!;
  });

  it("replaying a used payment header gets 402, no second delivery", async () => {
    const res = await postWithPayment(runUrl, JSON.stringify({ prompt: "hello there agent" }), paidHeader);
    expect(res.status).to.equal(402);
    expect(JSON.stringify(res.body)).to.include("receipt_not_pending");
  });

  it("client can rate after a paid call", async () => {
    await feedback({ client, agent, receipt: paidReceipt, score: 5 });
    const a = await program.account.agent.fetch(agent);
    expect(a.feedbackCount.toNumber()).to.equal(1);
  });

  // --------------------------------------------------------------- request binding

  it("payment header is bound to the exact body and the client key", async () => {
    const { reqs, receipt } = await payOnly();
    const bodyA = JSON.stringify({ prompt: "body A" });
    const bodyB = JSON.stringify({ prompt: "body B" });
    const header = (overrides: Partial<Parameters<typeof buildPaymentHeader>[0]> = {}) =>
      buildPaymentHeader({
        client,
        network: NETWORK,
        programId: program.programId,
        receipt,
        method: "POST",
        resource: reqs.resource,
        body: bodyA,
        ...overrides,
      });

    // signed for A, sent with B
    let res = await postWithPayment(runUrl, bodyB, header());
    expect(res.status).to.equal(402);
    expect(JSON.stringify(res.body)).to.include("bad client signature");

    // signed for another resource
    res = await postWithPayment(runUrl, bodyA, header({ resource: `${agentUrl}/api/other` }));
    expect(res.status).to.equal(402);

    // stale signature
    res = await postWithPayment(runUrl, bodyA, header({ issuedAt: Math.floor(Date.now() / 1000) - 3600 }));
    expect(res.status).to.equal(402);
    expect(JSON.stringify(res.body)).to.include("stale");

    // attacker signs the client's receipt with their own key
    const forged = buildPaymentHeader({
      client: attacker,
      network: NETWORK,
      programId: program.programId,
      receipt,
      method: "POST",
      resource: reqs.resource,
      body: bodyA,
    });
    res = await postWithPayment(runUrl, bodyA, forged);
    expect(res.status).to.equal(402);
    expect(JSON.stringify(res.body)).to.include("receipt_client_mismatch");

    // receipt untouched by all of the above
    const r = await fetchReceipt(program, receipt);
    expect(r && isPending(r)).to.equal(true);

    // the real request still works
    res = await postWithPayment(runUrl, bodyA, header());
    expect(res.status).to.equal(200);
  });

  it("rejects malformed payment headers", async () => {
    const body = JSON.stringify({ prompt: "x" });
    for (const h of ["not base64!!", Buffer.from("{}").toString("base64"), "A".repeat(5000)]) {
      const res = await postWithPayment(runUrl, body, h);
      expect(res.status).to.equal(402);
    }
  });

  // --------------------------------------------------------------- facilitator hardening

  describe("facilitator /verify", () => {
    let reqs: PaymentRequirements;
    let receipt: PublicKey;

    before(async () => {
      ({ reqs, receipt } = await payOnly());
    });

    const verify = (r: PaymentRequirements, p = payloadFor(receipt)) =>
      facilitator.verify({ x402Version: X402_VERSION, paymentPayload: p, paymentRequirements: r });

    it("accepts the real payment", async () => {
      expect((await verify(reqs)).isValid).to.equal(true);
    });

    it("rejects tampered requirements", async () => {
      expect((await verify({ ...reqs, maxAmountRequired: (PRICE + 1n).toString() })).invalidReason).to.equal(
        "insufficient_amount",
      );
      expect((await verify({ ...reqs, asset: attacker.publicKey.toBase58() })).invalidReason).to.equal(
        "invalid_asset",
      );
      expect((await verify({ ...reqs, payTo: attacker.publicKey.toBase58() })).invalidReason).to.equal(
        "invalid_pay_to",
      );
      expect(
        (await verify({ ...reqs, extra: { ...reqs.extra, agentId: "0" } })).invalidReason,
      ).to.equal("invalid_pay_to");
      expect((await verify({ ...reqs, network: "solana" })).invalidReason).to.equal("invalid_network");
      expect(
        (await verify({ ...reqs, extra: { ...reqs.extra, programId: attacker.publicKey.toBase58() } }))
          .invalidReason,
      ).to.equal("invalid_program");
      // needs more time than the receipt has left
      expect((await verify({ ...reqs, maxTimeoutSeconds: 86_400 })).invalidReason).to.equal(
        "receipt_expires_too_soon",
      );
    });

    it("rejects a receipt that is not a program account", async () => {
      const p = payloadFor(receipt);
      p.payload.receipt = treasury.toBase58();
      const out = await verify(reqs, p);
      expect(out.isValid).to.equal(false);
    });
  });

  describe("facilitator /settle", () => {
    let reqs: PaymentRequirements;
    let receipt: PublicKey;
    let feePayerBefore: number;

    before(async () => {
      ({ reqs, receipt } = await payOnly());
      feePayerBefore = await connection.getBalance(feePayer.publicKey, "confirmed");
    });

    const settleWith = (transaction: string) =>
      facilitator.settle({
        x402Version: X402_VERSION,
        paymentPayload: payloadFor(receipt),
        paymentRequirements: reqs,
        transaction,
      });

    it("refuses to pay for extra instructions (fee payer drain)", async () => {
      const drain = SystemProgram.transfer({
        fromPubkey: feePayer.publicKey,
        toPubkey: attacker.publicKey,
        lamports: 1_000_000_000,
      });
      const out = await settleWith(await settleTx({ receipt, extra: [drain] }));
      expect(out.errorReason).to.equal("unexpected_instructions");
    });

    it("refuses a different fee payer", async () => {
      const out = await settleWith(await settleTx({ receipt, payer: attacker.publicKey, signers: [attacker, operator] }));
      expect(out.errorReason).to.equal("invalid_fee_payer");
    });

    it("refuses a transaction the operator did not sign", async () => {
      const out = await settleWith(await settleTx({ receipt, signers: [] }));
      expect(out.errorReason).to.equal("invalid_operator_signature");
    });

    it("refuses a transaction signed by someone who is not the operator", async () => {
      const out = await settleWith(await settleTx({ receipt, operatorKey: attacker.publicKey, signers: [attacker] }));
      expect(out.errorReason).to.equal("invalid_operator");
    });

    it("refuses garbage", async () => {
      expect((await settleWith("AAAA")).success).to.equal(false);
      expect((await settleWith("A".repeat(5000))).errorReason).to.include("invalid_request");
    });

    it("rejected attempts did not touch the receipt or spend fees", async () => {
      const r = await fetchReceipt(program, receipt);
      expect(r && isPending(r)).to.equal(true);
      expect(await connection.getBalance(feePayer.publicKey, "confirmed")).to.equal(feePayerBefore);
    });

    it("settles the correct transaction exactly once", async () => {
      const out = await settleWith(await settleTx({ receipt }));
      expect(out.success, out.errorReason).to.equal(true);
      const again = await settleWith(await settleTx({ receipt }));
      expect(again.errorReason).to.equal("receipt_not_pending");
    });
  });

  // --------------------------------------------------------------- client-side checks

  it("client refuses to pay above its max", async () => {
    await expectError(
      paidCall({
        program: clientProgram(),
        client,
        network: NETWORK,
        url: runUrl,
        body: JSON.stringify({ prompt: "x" }),
        maxAmount: PRICE - 1n,
      }),
      "above your max",
    );
  });

  it("client refuses a quote whose resource is outside the agent endpoint", async () => {
    const reqs = await quote();
    await expectError(
      checkRequirements(clientProgram(), NETWORK, "http://evil.example/api/run", { ...reqs, resource: "http://evil.example/api/run" }, PRICE),
      "not under the agent's registered endpoint",
    );
  });

  it("inactive agent returns 503 and takes no payment", async () => {
    await program.methods.setAgentActive(false).accountsStrict({ owner: owner.publicKey, agent }).signers([owner]).rpc();
    const res = await postWithPayment(runUrl, JSON.stringify({ prompt: "x" }));
    expect(res.status).to.equal(503);
    await program.methods.setAgentActive(true).accountsStrict({ owner: owner.publicKey, agent }).signers([owner]).rpc();
  });

  it("price change after quote: client max protects the client on-chain", async () => {
    const reqs = await quote();
    const checked = await checkRequirements(clientProgram(), NETWORK, runUrl, reqs, PRICE);
    await program.methods
      .updateAgent({ endpoint: null, metadataUri: null, metadataHash: null, price: new BN(20_000), operator: null })
      .accountsStrict({ owner: owner.publicKey, config: CONFIG, agent })
      .signers([owner])
      .rpc();
    await expectError(clientCreatePayment(clientProgram(), client, reqs, checked), "PriceAboveMax");
    await program.methods
      .updateAgent({ endpoint: null, metadataUri: null, metadataHash: null, price: new BN(PRICE.toString()), operator: null })
      .accountsStrict({ owner: owner.publicKey, config: CONFIG, agent })
      .signers([owner])
      .rpc();
  });
});
