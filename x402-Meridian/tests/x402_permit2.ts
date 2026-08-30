import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotent,
  createMint,
  getAssociatedTokenAddressSync,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

import { Permit2 } from "../target/types/permit2";
import { X402 } from "../target/types/x402";
import { EthBuyer } from "./lib/ethSigner";
import { hashPermitWitnessTransferFrom, hashWitness, PermitTransferFromArgs, Witness } from "./lib/hashing";

const VAULT_SEED = Buffer.from("vault");
const VAULT_AUTHORITY_SEED = Buffer.from("vault-authority");
const NONCE_BITMAP_SEED = Buffer.from("nonce-bitmap");
const SETTLEMENT_AUTHORITY_SEED = Buffer.from("x402-settlement-authority");

describe("x402 + permit2 (Exact)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const connection: Connection = provider.connection;

  const permit2Program = anchor.workspace.Permit2 as Program<Permit2>;
  const x402Program = anchor.workspace.X402 as Program<X402>;

  const payer = (provider.wallet as anchor.Wallet).payer;
  const facilitator = Keypair.generate();
  const seller = Keypair.generate();

  const buyer = new EthBuyer(
    "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d",
  );

  let mint: PublicKey;
  let depositorTokenAccount: PublicKey;
  let sellerTokenAccount: PublicKey;

  let vaultPda: PublicKey;
  let vaultBump: number;
  let vaultTokenAccountPda: PublicKey;
  let settlementAuthorityPda: PublicKey;

  const DEPOSIT_AMOUNT = 1_000_000n;
  const PAY_AMOUNT = 250_000n;

  before(async () => {
    await connection.requestAirdrop(facilitator.publicKey, 2e9).then((sig) =>
      connection.confirmTransaction(sig, "confirmed"),
    );

    mint = await createMint(connection, payer, payer.publicKey, null, 6, undefined, undefined, TOKEN_PROGRAM_ID);

    depositorTokenAccount = await createAssociatedTokenAccountIdempotent(
      connection,
      payer,
      mint,
      payer.publicKey,
      undefined,
      TOKEN_PROGRAM_ID,
    );
    await mintTo(connection, payer, mint, depositorTokenAccount, payer, DEPOSIT_AMOUNT * 4n, [], undefined, TOKEN_PROGRAM_ID);

    sellerTokenAccount = await createAssociatedTokenAccountIdempotent(
      connection,
      payer,
      mint,
      seller.publicKey,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [VAULT_SEED, buyer.ethAddress, mint.toBuffer()],
      permit2Program.programId,
    );
    [vaultTokenAccountPda] = PublicKey.findProgramAddressSync(
      [VAULT_AUTHORITY_SEED, buyer.ethAddress, mint.toBuffer()],
      permit2Program.programId,
    );
    [settlementAuthorityPda] = PublicKey.findProgramAddressSync(
      [SETTLEMENT_AUTHORITY_SEED],
      x402Program.programId,
    );
  });

  it("initializes the vault for the buyer's eth address", async () => {
    await permit2Program.methods
      .initVault(Array.from(buyer.ethAddress))
      .accounts({
        payer: payer.publicKey,
        mint,
        vault: vaultPda,
        vaultTokenAccount: vaultTokenAccountPda,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const vault = await permit2Program.account.vault.fetch(vaultPda);
    assert.deepEqual(Array.from(vault.ethAddress), Array.from(buyer.ethAddress));
    assert.equal(vault.mint.toBase58(), mint.toBase58());
  });

  it("lets anyone deposit into the buyer's vault", async () => {
    await permit2Program.methods
      .deposit(Array.from(buyer.ethAddress), new anchor.BN(DEPOSIT_AMOUNT.toString()))
      .accounts({
        depositor: payer.publicKey,
        mint,
        vault: vaultPda,
        vaultTokenAccount: vaultTokenAccountPda,
        depositorTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const balance = await connection.getTokenAccountBalance(vaultTokenAccountPda);
    assert.equal(balance.value.amount, DEPOSIT_AMOUNT.toString());
  });

  it("settles an Exact x402 payment authorized by the buyer's eth signature, submitted by an unrelated facilitator", async () => {
    const nonce = 0n;
    const deadline = BigInt(Math.floor(Date.now() / 1000) + 3600);
    const validAfter = 0n;

    const permit: PermitTransferFromArgs = {
      permittedToken: mint,
      permittedAmount: PAY_AMOUNT,
      nonce,
      deadline,
    };
    const witness: Witness = {
      to: sellerTokenAccount,
      validAfter,
    };

    const witnessHash = hashWitness(witness);
    const dataHash = hashPermitWitnessTransferFrom(permit, settlementAuthorityPda, witnessHash);

    const secpIx = buyer.buildSecp256k1Instruction(dataHash, 0);

    const [nonceBitmapPda] = PublicKey.findProgramAddressSync(
      [NONCE_BITMAP_SEED, buyer.ethAddress, Buffer.from(new anchor.BN((nonce / 256n).toString()).toArray("le", 8))],
      permit2Program.programId,
    );

    const settleIx = await x402Program.methods
      .settle(
        {
          permittedToken: permit.permittedToken,
          permittedAmount: new anchor.BN(permit.permittedAmount.toString()),
          nonce: new anchor.BN(permit.nonce.toString()),
          deadline: new anchor.BN(permit.deadline.toString()),
        },
        Array.from(buyer.ethAddress),
        {
          to: witness.to,
          validAfter: new anchor.BN(witness.validAfter.toString()),
        },
      )
      .accounts({
        facilitator: facilitator.publicKey,
        settlementAuthority: settlementAuthorityPda,
        mint,
        vault: vaultPda,
        vaultTokenAccount: vaultTokenAccountPda,
        recipientTokenAccount: sellerTokenAccount,
        nonceBitmap: nonceBitmapPda,
        instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
        permit2Program: permit2Program.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .instruction();

    const tx = new Transaction().add(secpIx).add(settleIx);
    await anchor.web3.sendAndConfirmTransaction(connection, tx, [facilitator], { commitment: "confirmed" });

    const sellerBalance = await connection.getTokenAccountBalance(sellerTokenAccount);
    assert.equal(sellerBalance.value.amount, PAY_AMOUNT.toString());

    const vaultBalance = await connection.getTokenAccountBalance(vaultTokenAccountPda);
    assert.equal(vaultBalance.value.amount, (DEPOSIT_AMOUNT - PAY_AMOUNT).toString());
  });
});
