import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, PublicKey, SystemProgram } from "@solana/web3.js";
import {
  createAssociatedTokenAccountIdempotent,
  createMint,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { assert } from "chai";

import { Permit2 } from "../target/types/permit2";
import { EthBuyer } from "./lib/ethSigner";

const VAULT_SEED = Buffer.from("vault");
const VAULT_AUTHORITY_SEED = Buffer.from("vault-authority");

describe("permit2 (basic)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const connection: Connection = provider.connection;

  const program = anchor.workspace.Permit2 as Program<Permit2>;
  const payer = (provider.wallet as anchor.Wallet).payer;

  const buyer = new EthBuyer(
    "0x2222222222222222222222222222222222222222222222222222222222220001",
  );

  const DEPOSIT_AMOUNT = 500_000n;

  let mint: PublicKey;
  let depositorTokenAccount: PublicKey;
  let vaultPda: PublicKey;
  let vaultTokenAccountPda: PublicKey;

  before(async () => {
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

    depositorTokenAccount = await createAssociatedTokenAccountIdempotent(
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

    [vaultPda] = PublicKey.findProgramAddressSync(
      [VAULT_SEED, buyer.ethAddress, mint.toBuffer()],
      program.programId,
    );
    [vaultTokenAccountPda] = PublicKey.findProgramAddressSync(
      [VAULT_AUTHORITY_SEED, buyer.ethAddress, mint.toBuffer()],
      program.programId,
    );
  });

  it("init_vault stores the eth address, mint and token account", async () => {
    await program.methods
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

    const vault = await program.account.vault.fetch(vaultPda);
    assert.deepEqual(Array.from(vault.ethAddress), Array.from(buyer.ethAddress));
    assert.equal(vault.mint.toBase58(), mint.toBase58());
    assert.equal(vault.tokenAccount.toBase58(), vaultTokenAccountPda.toBase58());
  });

  it("deposit moves tokens into the vault token account", async () => {
    await program.methods
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

    const vaultBalance = await connection.getTokenAccountBalance(vaultTokenAccountPda);
    assert.equal(vaultBalance.value.amount, DEPOSIT_AMOUNT.toString());

    const depositorBalance = await connection.getTokenAccountBalance(depositorTokenAccount);
    assert.equal(depositorBalance.value.amount, DEPOSIT_AMOUNT.toString());
  });
});
