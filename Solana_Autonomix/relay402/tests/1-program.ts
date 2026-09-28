import { BN } from "@coral-xyz/anchor";
import { TOKEN_PROGRAM_ID, createMint } from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import { agentPda } from "../offchain/shared/pdas";
import {
  CONFIG,
  PROGRAM_DATA,
  PROGRAM_ID,
  VAULT,
  admin,
  ata,
  connection,
  createPayment,
  expectError,
  feedback,
  fetchConfigState,
  fund,
  newUser,
  program,
  refund,
  registerAgent,
  settle,
  tokenBalance,
  updateConfig,
  waitForChainTime,
} from "./helpers";

const PRICE = 10_000n;

describe("relay402: on-chain program", () => {
  let mint: PublicKey;
  let otherMint: PublicKey;
  let treasury: PublicKey;
  let owner: Keypair;
  let operator: Keypair;
  let client: Keypair;
  let attacker: Keypair;
  let payout: PublicKey;
  let clientToken: PublicKey;
  let attackerToken: PublicKey;
  let agent: PublicKey;
  let expiringReceipt: PublicKey;
  let expiringAt: number;

  const initAccounts = (signer: PublicKey, treasuryKey: PublicKey = treasury) => ({
    admin: signer,
    config: CONFIG,
    mint,
    vault: VAULT,
    treasury: treasuryKey,
    program: PROGRAM_ID,
    programData: PROGRAM_DATA,
    tokenProgram: TOKEN_PROGRAM_ID,
    systemProgram: SystemProgram.programId,
  });

  const settleArgs = (receipt: PublicKey) => ({
    operator,
    agent,
    receipt,
    mint,
    payout,
    treasury,
  });

  before(async () => {
    mint = await createMint(connection, admin, admin.publicKey, null, 6);
    otherMint = await createMint(connection, admin, admin.publicKey, null, 6);
    treasury = await ata(mint, admin.publicKey);
    [owner, operator, client, attacker] = await Promise.all([newUser(), newUser(), newUser(), newUser()]);
    payout = await ata(mint, owner.publicKey);
    clientToken = await fund(mint, client.publicKey, 1_000_000_000n);
    attackerToken = await fund(mint, attacker.publicKey, 1_000_000_000n);
  });

  // ---------------------------------------------------------------- config

  describe("initialize_config", () => {
    it("rejects a signer that is not the upgrade authority", async () => {
      await expectError(
        program.methods
          .initializeConfig(100, new BN(1000))
          .accountsStrict(initAccounts(attacker.publicKey, attackerToken))
          .signers([attacker])
          .rpc(),
        "Unauthorized",
      );
    });

    it("rejects fee above the cap and a zero min price", async () => {
      await expectError(
        program.methods.initializeConfig(1001, new BN(1000)).accountsStrict(initAccounts(admin.publicKey)).rpc(),
        "FeeTooHigh",
      );
      await expectError(
        program.methods.initializeConfig(100, new BN(0)).accountsStrict(initAccounts(admin.publicKey)).rpc(),
        "InvalidMinPrice",
      );
    });

    it("initializes", async () => {
      await program.methods.initializeConfig(100, new BN(1000)).accountsStrict(initAccounts(admin.publicKey)).rpc();
      const c = await fetchConfigState();
      expect(c.admin.toBase58()).to.equal(admin.publicKey.toBase58());
      expect(c.mint.toBase58()).to.equal(mint.toBase58());
      expect(c.vault.toBase58()).to.equal(VAULT.toBase58());
      expect(c.treasury.toBase58()).to.equal(treasury.toBase58());
      expect(c.feeBps).to.equal(100);
      expect(c.minPrice.toNumber()).to.equal(1000);
      expect(c.nextAgentId.toNumber()).to.equal(0);
      expect(c.paused).to.equal(false);
      expect(c.pendingAdmin).to.equal(null);
    });

    it("cannot be initialized twice", async () => {
      await expectError(
        program.methods.initializeConfig(100, new BN(1000)).accountsStrict(initAccounts(admin.publicKey)).rpc(),
        "already in use",
      );
    });
  });

  // ---------------------------------------------------------------- agents

  describe("agent registry", () => {
    it("rejects bad registrations", async () => {
      await expectError(
        registerAgent({ owner, operator: operator.publicKey, payout, price: 999 }),
        "PriceBelowMinimum",
      );
      await expectError(
        registerAgent({ owner, operator: operator.publicKey, payout, price: PRICE, endpoint: "" }),
        "InvalidEndpoint",
      );
      await expectError(
        registerAgent({ owner, operator: operator.publicKey, payout, price: PRICE, endpoint: "x".repeat(201) }),
        "InvalidEndpoint",
      );
      await expectError(
        registerAgent({ owner, operator: operator.publicKey, payout, price: PRICE, metadataUri: "" }),
        "InvalidMetadataUri",
      );
      await expectError(
        registerAgent({ owner, operator: PublicKey.default, payout, price: PRICE }),
        "InvalidOperator",
      );
      await expectError(
        registerAgent({ owner, operator: operator.publicKey, payout: VAULT, price: PRICE }),
        "InvalidPayout",
      );
      const wrongMint = await ata(otherMint, owner.publicKey);
      await expectError(
        registerAgent({ owner, operator: operator.publicKey, payout: wrongMint, price: PRICE }),
        "ConstraintTokenMint",
      );
    });

    it("registers an agent with a sequential id", async () => {
      const r = await registerAgent({ owner, operator: operator.publicKey, payout, price: PRICE });
      agent = r.agent;
      expect(r.id).to.equal(0n);
      const a = await program.account.agent.fetch(agent);
      expect(a.owner.toBase58()).to.equal(owner.publicKey.toBase58());
      expect(a.operator.toBase58()).to.equal(operator.publicKey.toBase58());
      expect(a.payout.toBase58()).to.equal(payout.toBase58());
      expect(a.price.toString()).to.equal(PRICE.toString());
      expect(a.active).to.equal(true);
      expect(a.pendingReceipts.toNumber()).to.equal(0);
      expect((await fetchConfigState()).nextAgentId.toNumber()).to.equal(1);
    });

    it("only the owner can update, with the same validation", async () => {
      const upd = (signer: Keypair, args: Record<string, unknown>) =>
        program.methods
          .updateAgent({ endpoint: null, metadataUri: null, metadataHash: null, price: null, operator: null, ...args })
          .accountsStrict({ owner: signer.publicKey, config: CONFIG, agent })
          .signers([signer])
          .rpc();
      await expectError(upd(attacker, { price: new BN(20_000) }), "Unauthorized");
      await expectError(upd(owner, { price: new BN(999) }), "PriceBelowMinimum");
      await expectError(upd(owner, { operator: PublicKey.default }), "InvalidOperator");
      await expectError(upd(owner, { endpoint: "" }), "InvalidEndpoint");
      await upd(owner, { price: new BN(20_000), endpoint: "https://agent2.example" });
      let a = await program.account.agent.fetch(agent);
      expect(a.price.toNumber()).to.equal(20_000);
      expect(a.endpoint).to.equal("https://agent2.example");
      await upd(owner, { price: new BN(PRICE.toString()) });
      a = await program.account.agent.fetch(agent);
      expect(a.price.toString()).to.equal(PRICE.toString());
    });

    it("set_payout validates the new account", async () => {
      const setPayout = (signer: Keypair, p: PublicKey) =>
        program.methods
          .setPayout()
          .accountsStrict({ owner: signer.publicKey, config: CONFIG, agent, payout: p })
          .signers([signer])
          .rpc();
      await expectError(setPayout(attacker, attackerToken), "Unauthorized");
      await expectError(setPayout(owner, VAULT), "InvalidPayout");
      await setPayout(owner, payout);
    });
  });

  // ---------------------------------------------------------------- payments

  describe("payments", () => {
    let receipt: PublicKey;

    it("creates a short-window receipt used later for the refund path", async () => {
      expiringReceipt = await createPayment({ client, agent, mint, nonce: 999n, windowSecs: 60 });
      const r = await program.account.receipt.fetch(expiringReceipt);
      expiringAt = r.expiresAt.toNumber();
      await expectError(refund({ client, agent, receipt: expiringReceipt, mint }), "PaymentNotExpired");
    });

    it("rejects bad payment parameters", async () => {
      await expectError(createPayment({ client, agent, mint, nonce: 1n, windowSecs: 59 }), "InvalidPaymentWindow");
      await expectError(
        createPayment({ client, agent, mint, nonce: 1n, windowSecs: 86_401 }),
        "InvalidPaymentWindow",
      );
      await expectError(createPayment({ client, agent, mint, nonce: 1n, maxAmount: PRICE - 1n }), "PriceAboveMax");
      // paying from someone else's token account
      await expectError(createPayment({ client, agent, mint, nonce: 1n, clientToken: attackerToken }), "ConstraintTokenOwner");
    });

    it("creates a payment and moves funds into escrow", async () => {
      const clientBefore = await tokenBalance(clientToken);
      const vaultBefore = await tokenBalance(VAULT);
      receipt = await createPayment({ client, agent, mint, nonce: 1n, maxAmount: PRICE });
      expect(await tokenBalance(clientToken)).to.equal(clientBefore - PRICE);
      expect(await tokenBalance(VAULT)).to.equal(vaultBefore + PRICE);
      const r = await program.account.receipt.fetch(receipt);
      expect(r.amount.toString()).to.equal(PRICE.toString());
      expect(r.feeBps).to.equal(100);
      expect(r.status).to.deep.equal({ pending: {} });
      expect(r.expiresAt.toNumber() - r.createdAt.toNumber()).to.equal(3600);
      expect((await program.account.agent.fetch(agent)).pendingReceipts.toNumber()).to.equal(2);
    });

    it("rejects reusing an open nonce", async () => {
      await expectError(createPayment({ client, agent, mint, nonce: 1n }), "already in use");
    });

    it("only the operator can settle, with the registered payout and the treasury", async () => {
      await expectError(settle({ ...settleArgs(receipt), operator: attacker }), "Unauthorized");
      await expectError(settle({ ...settleArgs(receipt), payout: attackerToken }), "InvalidPayout");
      await expectError(settle({ ...settleArgs(receipt), treasury: attackerToken }), "InvalidTreasury");
      // the owner is not the operator either
      await expectError(settle({ ...settleArgs(receipt), operator: owner }), "Unauthorized");
    });

    it("refund of a pending receipt is not allowed before expiry", async () => {
      await expectError(refund({ client, agent, receipt, mint }), "PaymentNotExpired");
    });

    it("settles: agent gets price minus fee, treasury gets the fee", async () => {
      const payoutBefore = await tokenBalance(payout);
      const treasuryBefore = await tokenBalance(treasury);
      const vaultBefore = await tokenBalance(VAULT);
      await settle(settleArgs(receipt));
      expect(await tokenBalance(payout)).to.equal(payoutBefore + 9_900n);
      expect(await tokenBalance(treasury)).to.equal(treasuryBefore + 100n);
      expect(await tokenBalance(VAULT)).to.equal(vaultBefore - PRICE);
      const r = await program.account.receipt.fetch(receipt);
      expect(r.status).to.deep.equal({ settled: {} });
      const a = await program.account.agent.fetch(agent);
      expect(a.pendingReceipts.toNumber()).to.equal(1);
      expect(a.settledCount.toNumber()).to.equal(1);
    });

    it("cannot settle or refund twice", async () => {
      await expectError(settle(settleArgs(receipt)), "PaymentNotPending");
      await expectError(refund({ client, agent, receipt, mint }), "PaymentNotPending");
    });

    it("feedback: range checked, only the paying client, exactly once", async () => {
      await expectError(feedback({ client, agent, receipt, score: 0 }), "InvalidScore");
      await expectError(feedback({ client, agent, receipt, score: 6 }), "InvalidScore");
      // attacker cannot use the client's receipt (PDA seeds include the signer)
      await expectError(feedback({ client: attacker, agent, receipt, score: 1 }), "ConstraintSeeds");
      await feedback({ client, agent, receipt, score: 4 });
      const a = await program.account.agent.fetch(agent);
      expect(a.feedbackCount.toNumber()).to.equal(1);
      expect(a.scoreSum.toNumber()).to.equal(4);
      expect(await connection.getAccountInfo(receipt)).to.equal(null);
      await expectError(feedback({ client, agent, receipt, score: 5 }), "AccountNotInitialized");
    });

    it("feedback on a pending receipt is rejected", async () => {
      const pending = await createPayment({ client, agent, mint, nonce: 2n });
      await expectError(feedback({ client, agent, receipt: pending, score: 5 }), "PaymentNotSettled");
      await settle(settleArgs(pending));
      // skip feedback, reclaim rent
      await program.methods
        .closeReceipt()
        .accountsStrict({ client: client.publicKey, receipt: pending })
        .signers([client])
        .rpc();
      expect(await connection.getAccountInfo(pending)).to.equal(null);
    });

    it("close_receipt only works for the client on a settled receipt", async () => {
      const pending = await createPayment({ client, agent, mint, nonce: 3n });
      const close = (signer: Keypair) =>
        program.methods
          .closeReceipt()
          .accountsStrict({ client: signer.publicKey, receipt: pending })
          .signers([signer])
          .rpc();
      await expectError(close(client), "PaymentNotSettled");
      await settle(settleArgs(pending));
      await expectError(close(attacker), "ReceiptClientMismatch");
      await close(client);
    });

    it("agent owner cannot rate their own agent", async () => {
      await fund(mint, owner.publicKey, 1_000_000n); // owner's ATA is also the payout
      const own = await createPayment({ client: owner, agent, mint, nonce: 1n });
      await settle(settleArgs(own));
      await expectError(feedback({ client: owner, agent, receipt: own, score: 5 }), "SelfFeedback");
    });

    it("uses the fee rate snapshot from payment time", async () => {
      const r = await createPayment({ client, agent, mint, nonce: 4n });
      await updateConfig(admin, { feeBps: 500 });
      const before = await tokenBalance(treasury);
      await settle(settleArgs(r));
      expect(await tokenBalance(treasury)).to.equal(before + 100n); // 1%, not 5%
      await updateConfig(admin, { feeBps: 100 });
    });

    it("rounds the fee up", async () => {
      const upd = (price: number) =>
        program.methods
          .updateAgent({ endpoint: null, metadataUri: null, metadataHash: null, price: new BN(price), operator: null })
          .accountsStrict({ owner: owner.publicKey, config: CONFIG, agent })
          .signers([owner])
          .rpc();
      await upd(1001);
      const r = await createPayment({ client, agent, mint, nonce: 5n });
      const t0 = await tokenBalance(treasury);
      const p0 = await tokenBalance(payout);
      await settle(settleArgs(r));
      expect(await tokenBalance(treasury)).to.equal(t0 + 11n); // ceil(10.01)
      expect(await tokenBalance(payout)).to.equal(p0 + 990n);
      await upd(Number(PRICE));
    });

    it("pause blocks new payments and agents but not settlement", async () => {
      const r = await createPayment({ client, agent, mint, nonce: 6n });
      await updateConfig(admin, { paused: true });
      await expectError(createPayment({ client, agent, mint, nonce: 7n }), "Paused");
      await expectError(registerAgent({ owner, operator: operator.publicKey, payout, price: PRICE }), "Paused");
      await settle(settleArgs(r));
      await updateConfig(admin, { paused: false });
    });

    it("raising min_price blocks payments to agents priced below it", async () => {
      await updateConfig(admin, { minPrice: Number(PRICE) + 1 });
      await expectError(createPayment({ client, agent, mint, nonce: 8n }), "PriceBelowMinimum");
      await updateConfig(admin, { minPrice: 1000 });
    });

    it("expired receipt: settle fails, refund returns the full amount", async () => {
      await waitForChainTime(expiringAt);
      await expectError(settle(settleArgs(expiringReceipt)), "PaymentExpired");
      // someone else cannot refund to themselves
      await expectError(
        refund({ client: attacker, agent, receipt: expiringReceipt, mint, clientToken: attackerToken }),
        "ConstraintSeeds",
      );
      const before = await tokenBalance(clientToken);
      const vaultBefore = await tokenBalance(VAULT);
      await refund({ client, agent, receipt: expiringReceipt, mint });
      expect(await tokenBalance(clientToken)).to.equal(before + PRICE);
      expect(await tokenBalance(VAULT)).to.equal(vaultBefore - PRICE);
      expect(await connection.getAccountInfo(expiringReceipt)).to.equal(null);
      expect((await program.account.agent.fetch(agent)).pendingReceipts.toNumber()).to.equal(0);
    });

    it("vault holds exactly the pending escrow (nothing left over)", async () => {
      expect(await tokenBalance(VAULT)).to.equal(0n);
    });
  });

  // ---------------------------------------------------------------- admin

  describe("admin controls", () => {
    it("rejects non-admin and out-of-range updates", async () => {
      await expectError(updateConfig(attacker, { paused: true }), "Unauthorized");
      await expectError(updateConfig(admin, { feeBps: 1001 }), "FeeTooHigh");
      await expectError(updateConfig(admin, { minPrice: 0 }), "InvalidMinPrice");
    });

    it("set_treasury rejects the vault and a wrong mint", async () => {
      const setT = (t: PublicKey) =>
        program.methods.setTreasury().accountsStrict({ admin: admin.publicKey, config: CONFIG, treasury: t }).rpc();
      await expectError(setT(VAULT), "InvalidTreasury");
      await expectError(setT(await ata(otherMint, admin.publicKey)), "ConstraintTokenMint");
      await setT(treasury);
    });

    it("rotates admin in two steps", async () => {
      const next = await newUser(1);
      await expectError(
        program.methods.proposeAdmin(PublicKey.default).accountsStrict({ admin: admin.publicKey, config: CONFIG }).rpc(),
        "InvalidAuthority",
      );
      await program.methods.proposeAdmin(next.publicKey).accountsStrict({ admin: admin.publicKey, config: CONFIG }).rpc();
      const accept = (s: Keypair) =>
        program.methods.acceptAdmin().accountsStrict({ newAdmin: s.publicKey, config: CONFIG }).signers([s]).rpc();
      await expectError(accept(attacker), "NoPendingTransfer");
      await accept(next);
      await expectError(updateConfig(admin, { paused: false }), "Unauthorized");
      // hand it back so later suites keep working
      await program.methods
        .proposeAdmin(admin.publicKey)
        .accountsStrict({ admin: next.publicKey, config: CONFIG })
        .signers([next])
        .rpc();
      await program.methods.acceptAdmin().accountsStrict({ newAdmin: admin.publicKey, config: CONFIG }).rpc();
      const c = await fetchConfigState();
      expect(c.admin.toBase58()).to.equal(admin.publicKey.toBase58());
      expect(c.pendingAdmin).to.equal(null);
    });
  });

  // ---------------------------------------------------------------- lifecycle

  describe("agent lifecycle", () => {
    it("transfers ownership in two steps", async () => {
      const next = await newUser(1);
      await program.methods
        .transferAgent(next.publicKey)
        .accountsStrict({ owner: owner.publicKey, agent })
        .signers([owner])
        .rpc();
      const accept = (s: Keypair) =>
        program.methods.acceptAgent().accountsStrict({ newOwner: s.publicKey, agent }).signers([s]).rpc();
      await expectError(accept(attacker), "NoPendingTransfer");
      await accept(next);
      await expectError(
        program.methods.setAgentActive(false).accountsStrict({ owner: owner.publicKey, agent }).signers([owner]).rpc(),
        "Unauthorized",
      );
      await program.methods
        .transferAgent(owner.publicKey)
        .accountsStrict({ owner: next.publicKey, agent })
        .signers([next])
        .rpc();
      await accept(owner);
    });

    it("cannot close while active or with pending payments", async () => {
      const close = () =>
        program.methods.closeAgent().accountsStrict({ owner: owner.publicKey, agent }).signers([owner]).rpc();
      await expectError(close(), "AgentNotClosable");

      const pending = await createPayment({ client, agent, mint, nonce: 50n });
      await program.methods.setAgentActive(false).accountsStrict({ owner: owner.publicKey, agent }).signers([owner]).rpc();
      await expectError(createPayment({ client, agent, mint, nonce: 51n }), "AgentInactive");
      await expectError(close(), "AgentNotClosable");

      // an inactive agent can still settle what was already paid
      await settle(settleArgs(pending));
      await close();
      expect(await connection.getAccountInfo(agent)).to.equal(null);
    });

    it("never reuses an agent id", async () => {
      const before = (await fetchConfigState()).nextAgentId.toNumber();
      const r = await registerAgent({ owner, operator: operator.publicKey, payout, price: PRICE });
      expect(Number(r.id)).to.equal(before);
      expect(r.agent.toBase58()).to.equal(agentPda(BigInt(before), PROGRAM_ID).toBase58());
      expect(r.agent.toBase58()).to.not.equal(agent.toBase58());
    });
  });
});
