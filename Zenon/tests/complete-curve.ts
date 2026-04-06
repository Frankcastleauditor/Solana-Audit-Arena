import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import { Tokens } from '../target/types/tokens';
import { assert } from 'chai';
import { TransactionInstruction } from '@solana/web3.js';

import {
  MOCK_TOKEN_METADATA_10_000,
  TRADING_CONFIG_10_000,
  MARKET_CONFIG,
  airdropAndWait,
  initializeRandomToken,
  tradeAndAssert,
  withAtaInitInstruction,
} from './helpers';
import { getAccount, getAssociatedTokenAddressSync } from '@solana/spl-token';

describe('Complete bonding curve', async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const trader = (provider.wallet as anchor.Wallet).payer;
  const program = anchor.workspace.Tokens as Program<Tokens>;

  it('Should withdraw reserves', async () => {
    const ctx = await initializeRandomToken(
      program,
      trader,
      MOCK_TOKEN_METADATA_10_000
    );
    const {
      marketData,
      mint,
      bondingCurveAta,
      bondingCurvePda,
      marketVersion,
    } = ctx;
    const { authority, escapeFeeTreasury } = marketData;
    // Buy the first block of tokens
    const FIRST_BUY_AMOUNT = new anchor.BN('800000000711270');
    const FIRST_BUY_PRICE = new anchor.BN('55555575800');
    const buy1stTokens = await tradeAndAssert(
      provider,
      ctx,
      trader,
      true,
      FIRST_BUY_AMOUNT,
      FIRST_BUY_PRICE,
      1,
      {
        bondingCurveRealSolReserves: FIRST_BUY_PRICE,
        bondingCurveRealTokenReserves:
          marketData.initialMint.sub(FIRST_BUY_AMOUNT),
        bondingCurveVirtualSolReserves: FIRST_BUY_PRICE.add(
          TRADING_CONFIG_10_000.SOL_OFFSET
        ),
        bondingCurveVirtualTokenReserves: marketData.initialMint
          .sub(FIRST_BUY_AMOUNT)
          .add(TRADING_CONFIG_10_000.TOKENS_OFFSET),
        bondingCurveTokenSupply: marketData.initialMint,
        traderTokenBalance: FIRST_BUY_AMOUNT,
      }
    );
    const { bondingCurveAccount } = buy1stTokens;
    console.log(`Escaped curve: ${bondingCurveAccount.completed}`);
    assert.equal(
      bondingCurveAccount.completed,
      true,
      'Should have escape curve'
    );
    const bondingCurveSolBalanceBefore = await provider.connection.getBalance(
      bondingCurvePda
    );
    const airdropAmount = 100000000000;
    await airdropAndWait(provider, authority.publicKey, airdropAmount);
    const { ata: adminAta, preInstructions } = await withAtaInitInstruction(
      provider,
      mint.publicKey,
      authority
    );
    const escapeFeeTreasuryBalanceBefore = await provider.connection.getBalance(
      escapeFeeTreasury
    );
    await program.methods
      .processCompletedCurve(marketVersion)
      .preInstructions(preInstructions)
      .accounts({
        mint: mint.publicKey,
        admin: authority.publicKey,
        escapeFeeTreasury,
      })
      .signers([authority])
      .rpc();
    // Admin token balance
    const adminExpectedTokenBalance = marketData.initialMint
      .sub(FIRST_BUY_AMOUNT)
      .sub(MARKET_CONFIG.tokensFeeAmount);
    const adminAtaAccount = await getAccount(provider.connection, adminAta);
    console.log(
      `Admin balance: ${
        adminAtaAccount.amount
      }, expected: ${adminExpectedTokenBalance.toString()}`
    );
    assert.equal(
      adminExpectedTokenBalance.toString(),
      adminAtaAccount.amount.toString(),
      'Token reserves should be in admin account'
    );
    // The fee should remain in the bonding curve
    const updatedBondingCurveAccount = await getAccount(
      provider.connection,
      bondingCurveAta
    );
    console.log(
      `Bonding curve token balance: ${
        updatedBondingCurveAccount.amount
      }, expected: ${MARKET_CONFIG.tokensFeeAmount.toString()}`
    );
    assert.equal(
      updatedBondingCurveAccount.amount.toString(),
      MARKET_CONFIG.tokensFeeAmount.toString(),
      'Token reserves should be in admin account'
    );
    // Escape fee treasury balance
    const escapeFeeTreasuryBalanceAfter = await provider.connection.getBalance(
      escapeFeeTreasury
    );
    const escapeFeeTreasuryBalanceIncrease =
      escapeFeeTreasuryBalanceAfter - escapeFeeTreasuryBalanceBefore;
    const escapeFee = FIRST_BUY_PRICE.muln(MARKET_CONFIG.escapeFeeBps).divRound(
      new anchor.BN(10000)
    );
    console.log(
      `Escape fee treasury balance increase: ${escapeFeeTreasuryBalanceIncrease}, expected: ${escapeFee}`
    );
    assert.equal(
      escapeFeeTreasuryBalanceIncrease.toString(),
      escapeFee.toString(),
      'Escape fee treasury balance should increase'
    );
    // Admin SOL balance increase
    const adminSolBalance = await provider.connection.getBalance(
      authority.publicKey
    );
    const adminSolBalanceIncrease = adminSolBalance - airdropAmount;
    // TODO: get the actual fee
    const aproxSolFee = new anchor.BN(3_000_000);
    const minExpectedSolIncrease =
      FIRST_BUY_PRICE.sub(aproxSolFee).sub(escapeFee);
    console.log(
      `Admin SOL balance increase: ${adminSolBalanceIncrease}, min expected: ${minExpectedSolIncrease}`
    );
    assert.isAtLeast(
      adminSolBalanceIncrease,
      minExpectedSolIncrease.toNumber(),
      'Admin SOL balance should increase'
    );
    // Bonding curve SOL reserves
    const bondingCurveSolBalanceAfter = await provider.connection.getBalance(
      bondingCurvePda
    );
    const bondingCurveSolBalanceDecrease =
      bondingCurveSolBalanceBefore - bondingCurveSolBalanceAfter;
    console.log(
      `Bonding curve SOL decreased: ${bondingCurveSolBalanceDecrease}, expected: ${FIRST_BUY_PRICE}`
    );
    assert.equal(
      FIRST_BUY_PRICE.toString(),
      bondingCurveSolBalanceDecrease.toString(),
      'Bonding curve SOL reserves have decreased'
    );
  });

  it('Should block funds withdrawal when curve is incomplete', async () => {
    const ctx = await initializeRandomToken(
      program,
      trader,
      MOCK_TOKEN_METADATA_10_000
    );
    const { marketData, mint, marketVersion } = ctx;
    const { authority } = marketData;
    // Buy the first block of tokens
    const FIRST_BUY_AMOUNT = new anchor.BN('799999999932903');
    const FIRST_BUY_PRICE = new anchor.BN(55_555_575_600);
    const buy1stTokens = await tradeAndAssert(
      provider,
      ctx,
      trader,
      true,
      FIRST_BUY_AMOUNT,
      FIRST_BUY_PRICE,
      2,
      {
        bondingCurveRealSolReserves: FIRST_BUY_PRICE,
        bondingCurveRealTokenReserves:
          marketData.initialMint.sub(FIRST_BUY_AMOUNT),
        bondingCurveVirtualSolReserves: FIRST_BUY_PRICE.add(
          TRADING_CONFIG_10_000.SOL_OFFSET
        ),
        bondingCurveVirtualTokenReserves: marketData.initialMint
          .sub(FIRST_BUY_AMOUNT)
          .add(TRADING_CONFIG_10_000.TOKENS_OFFSET),
        bondingCurveTokenSupply: marketData.initialMint,
        traderTokenBalance: FIRST_BUY_AMOUNT,
      }
    );
    const { bondingCurveAccount } = buy1stTokens;
    console.log(`Escaped curve: ${bondingCurveAccount.completed}`);
    assert.equal(
      bondingCurveAccount.completed,
      false,
      'Should not have escape curve'
    );
    try {
      const airdropAmount = 100000000000;
      await airdropAndWait(provider, authority.publicKey, airdropAmount);
      let preInstructions: Array<TransactionInstruction> = [];
      const adminAta = getAssociatedTokenAddressSync(
        mint.publicKey,
        authority.publicKey,
        true
      );
      const initAdminAtaInst = await program.methods
        .initAta()
        .accounts({
          mint: mint.publicKey,
          payer: authority.publicKey,
          authority: authority.publicKey,
        })
        .signers([authority])
        .instruction();
      preInstructions.push(initAdminAtaInst);
      await program.methods
        .processCompletedCurve(marketVersion)
        .preInstructions(preInstructions)
        .accounts({
          mint: mint.publicKey,
          admin: authority.publicKey,
          escapeFeeTreasury: marketData.escapeFeeTreasury,
        })
        .signers([authority])
        .rpc();
      assert.fail('Should have failed');
    } catch (err) {
      assert.equal(
        'BondingCurveNotCompleted',
        err.error?.errorCode?.code,
        'BondingCurveNotCompleted error when buying'
      );
    }
  });
});
