import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  Transaction,
  TransactionInstruction,
} from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotent,
  createMint,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

import { Permit2 } from "../target/types/permit2";
import { X402 } from "../target/types/x402";
import { EthBuyer } from "./lib/ethSigner";
import {
  hashPermitWitnessTransferFrom,
  hashWitness,
  PermitTransferFromArgs,
  Witness,
} from "./lib/hashing";

const VAULT_SEED = Buffer.from("vault");
const VAULT_AUTHORITY_SEED = Buffer.from("vault-authority");
const NONCE_BITMAP_SEED = Buffer.from("nonce-bitmap");
const SETTLEMENT_AUTHORITY_SEED = Buffer.from("x402-settlement-authority");

describe("x402 (basic)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const connection: Connection = provider.connection;

  const permit2Program = anchor.workspace.Permit2 as Program<Permit2>;
  const x402Program = anchor.workspace.X402 as Program<X402>;

  const payer = (provider.wallet as anchor.Wallet).payer;
  const facilitator = Keypair.generate();
  const seller = Keypair.generate();

  const buyer = new EthBuyer(
    "0x3333333333333333333333333333333333333333333333333333333333330002",
  );

  const DEPOSIT_AMOUNT = 1_000_000n;
  const PAY_AMOUNT = 100_000n;

  let mint: PublicKey;
  let sellerTokenAccount: PublicKey;
  let vaultPda: PublicKey;
  let vaultTokenAccountPda: PublicKey;
  let settlementAuthorityPda: PublicKey;

  async function buildSettleTx(nonce: bigint, validAfter: bigint): Promise<Transaction> {
    const permit: PermitTransferFromArgs = {
      permittedToken: mint,
      permittedAmount: PAY_AMOUNT,
      nonce,
      deadline: BigInt(Math.floor(Date.now() / 1000) + 3600),
    };
    const witness: Witness = { to: sellerTokenAccount, validAfter };

    const dataHash = hashPermitWitnessTransferFrom(
      permit,
      settlementAuthorityPda,
      hashWitness(witness),
    );
    const secpIx: TransactionInstruction = buyer.buildSecp256k1Instruction(dataHash, 0);

    const [nonceBitmapPda] = PublicKey.findProgramAddressSync(
      [
        NONCE_BITMAP_SEED,
        buyer.ethAddress,
        Buffer.from(new anchor.BN((nonce / 256n).toString()).toArray("le", 8)),
      ],
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
        { to: witness.to, validAfter: new anchor.BN(witness.validAfter.toString()) },
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

    return new Transaction().add(secpIx).add(settleIx);
  }

  before(async () => {
    const sig = await connection.requestAirdrop(facilitator.publicKey, 2e9);
    await connection.confirmTransaction(sig, "confirmed");

    mint = await createMint(
      connection,
      payer,
      payer.publicKey,
      null,
      6,
      undefined,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    const depositorTokenAccount = await createAssociatedTokenAccountIdempotent(
      connection,
      payer,
      mint,
      payer.publicKey,
      undefined,
      TOKEN_PROGRAM_ID,
    );
    await mintTo(
      connection,
      payer,
      mint,
      depositorTokenAccount,
      payer,
      DEPOSIT_AMOUNT * 2n,
      [],
      undefined,
      TOKEN_PROGRAM_ID,
    );

    sellerTokenAccount = await createAssociatedTokenAccountIdempotent(
      connection,
      payer,
      mint,
      seller.publicKey,
      undefined,
      TOKEN_PROGRAM_ID,
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
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
  });

  it("settle pays the seller from the buyer's vault", async () => {
    const tx = await buildSettleTx(0n, 0n);
    await anchor.web3.sendAndConfirmTransaction(connection, tx, [facilitator], {
      commitment: "confirmed",
    });

    const sellerBalance = await connection.getTokenAccountBalance(sellerTokenAccount);
    assert.equal(sellerBalance.value.amount, PAY_AMOUNT.toString());

    const vaultBalance = await connection.getTokenAccountBalance(vaultTokenAccountPda);
    assert.equal(vaultBalance.value.amount, (DEPOSIT_AMOUNT - PAY_AMOUNT).toString());
  });
});
