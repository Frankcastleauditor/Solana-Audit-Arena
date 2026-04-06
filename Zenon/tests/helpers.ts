import * as anchor from '@coral-xyz/anchor';
import { TransactionInstruction } from '@solana/web3.js';
import { Keypair } from '@solana/web3.js';
import { Program } from '@coral-xyz/anchor';
import { Tokens } from '../target/types/tokens';
import { PublicKey } from '@solana/web3.js';
import { Buffer } from 'buffer';
import { getAccount, getAssociatedTokenAddressSync } from '@solana/spl-token';
import { assert } from 'chai';

export const TOKEN_METADATA_PROGRAM_ID = new PublicKey(
  'metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s'
);

export const ONE_YEAR_FROM_NOW_TS = Math.round(
  new Date().getTime() / 1000 + 365 * 24 * 60 * 60
);
export const NOW_TS = Math.round(new Date().getTime() / 1000);

export const TRADING_CONFIG_10_000 = {
  TOKENS_OFFSET: new anchor.BN(96_290_613_000_000),
  SOL_OFFSET: new anchor.BN(20_575_744_444),
  FIRST_BUY_AMOUNT: new anchor.BN(99_999_999_999_711),
  FIRST_BUY_PRICE: new anchor.BN(2_065_235_201),
  SECOND_BUY_AMOUNT: new anchor.BN(100_000_000_001_066),
  SECOND_BUY_PRICE: new anchor.BN(2_526_075_730),
  FIRST_SELL_AMOUNT: new anchor.BN(99_999_999_957_064),
  FIRST_SELL_PRICE: new anchor.BN(2_526_075_729),
  SECOND_SELL_AMOUNT: new anchor.BN(99_999_999_990_435),
  SECOND_SELL_PRICE: new anchor.BN(2_065_235_201),
};

export const ONE_TRADER_TRADING_CONFIG_10_000 = {
  TOKENS_OFFSET: new anchor.BN(96_290_613_000_000),
  SOL_OFFSET: new anchor.BN(20_575_744_444),
  FIRST_BUY_AMOUNT: new anchor.BN(99_999_999_999_711),
  FIRST_BUY_PRICE: new anchor.BN(2_065_235_201),
  FIRST_SELL_AMOUNT: new anchor.BN(99_999_999_946_431),
  FIRST_SELL_PRICE: new anchor.BN(2_065_235_200),
};

export const MARKET_CONFIG = {
  initialMint: new anchor.BN(1_000_000_000_000_000),
  escapeAmount: new anchor.BN(800_000_000_000_000),
  escapeFeeBps: 1069,
  tokensFeeAmount: new anchor.BN(6_900_000_000_000),
  tradingFeeBps: 100,
};

export const MARKET_CONFIG_2 = {
  initialMint: new anchor.BN(1_000_000_000_000_069),
  escapeAmount: new anchor.BN(800_000_000_000_069),
  escapeFeeBps: 169,
  tokensFeeAmount: new anchor.BN(6_900_000_000_069),
  tradingFeeBps: 69,
};

export const MOCK_TOKEN_METADATA_10_000: Parameters<
  Program<Tokens>['methods']['initToken']
>[0] = {
  decimals: 6,
  name: 'Test Token',
  symbol: 'TT',
  uri: 'https://test.com',
  tokensFeeCooldownTimestamp: new anchor.BN(ONE_YEAR_FROM_NOW_TS),
  solOffset: TRADING_CONFIG_10_000.SOL_OFFSET,
  tokenOffset: TRADING_CONFIG_10_000.TOKENS_OFFSET,
};

export function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export const createMarket = async (
  program: Program<Tokens>,
  payer: anchor.web3.Keypair,
  version: number,
  market: Parameters<Program<Tokens>['methods']['initializeMarket']>[1]
) => {
  const buf = Buffer.alloc(2);
  buf.writeUInt16LE(version);
  const seeds = [
    Buffer.from('market'), // Convert "market" string to Buffer
    buf,
  ];
  const [marketPda, marketBump] = PublicKey.findProgramAddressSync(
    seeds,
    program.programId
  );
  const tx = await program.methods
    .initializeMarket(version, market)
    .signers([payer])
    .accounts({
      payer: payer.publicKey,
    })
    .rpc();
  console.log('Init Market TX', tx);
  const onChainMarketAccount = await program.account.market.fetch(marketPda);
  console.log(`Market PDA: ${marketPda.toString()} (version - ${version})`);
  return {
    onChainMarketAccount,
    marketPda,
    marketBump,
    tx,
  };
};

export const generateRandomMarketData = () => {
  const marketVersion = Math.floor(Math.random() * 65535);
  const authority = Keypair.generate();
  const escapeFeeTreasury = Keypair.generate().publicKey;
  const tokensFeeTreasury = Keypair.generate().publicKey;
  const tradingFeeTreasury = Keypair.generate().publicKey;
  const marketData = {
    authority,
    escapeFeeTreasury,
    tokensFeeTreasury,
    tradingFeeTreasury,
    ...MARKET_CONFIG,
  };
  return { marketData, authority, marketVersion };
};
export type GeneratedTokenContext = Awaited<
  Promise<ReturnType<typeof initializeRandomToken>>
>;
export const initializeRandomToken = async (
  program: Program<Tokens>,
  payer: anchor.web3.Keypair,
  metadata: Parameters<Program<Tokens>['methods']['initToken']>[0]
) => {
  const { marketData, marketVersion } = generateRandomMarketData();
  const { marketPda, marketBump } = await createMarket(
    program,
    payer,
    marketVersion,
    {
      ...marketData,
      authority: marketData.authority.publicKey,
    }
  );
  const mint = Keypair.generate();
  console.log(`Mint: ${mint.publicKey}`);
  const [bondingCurvePda, bondingCurveBump] = PublicKey.findProgramAddressSync(
    [Buffer.from('bonding_curve'), mint.publicKey.toBuffer()],
    program.programId
  );
  console.log(`Bonding Curve PDA: ${bondingCurvePda}`);
  const bondingCurveAta = getAssociatedTokenAddressSync(
    mint.publicKey,
    bondingCurvePda,
    true
  );
  console.log(`Bonding Curve ATA: ${bondingCurveAta}`);
  console.log(`Buyer ATA: ${bondingCurveAta}`);
  console.log(`Second Buyer ATA: ${bondingCurveAta}`);
  const [metadataAddress] = PublicKey.findProgramAddressSync(
    [
      Buffer.from('metadata'),
      TOKEN_METADATA_PROGRAM_ID.toBuffer(),
      mint.publicKey.toBuffer(),
    ],
    TOKEN_METADATA_PROGRAM_ID
  );
  const tx = await program.methods
    .initToken(metadata)
    .signers([payer, mint])
    .accounts({
      payer: payer.publicKey,
      mint: mint.publicKey,
      metadata: metadataAddress,
      market: marketPda,
    })
    .rpc();
  console.log('TX', tx);
  return {
    mint,
    bondingCurvePda,
    bondingCurveAta,
    metadataAddress,
    bondingCurveBump,
    tx,
    marketData,
    marketPda,
    marketBump,
    marketVersion,
  };
};

export const tradeAndAssert = async (
  provider: anchor.AnchorProvider,
  ctx: GeneratedTokenContext,
  payer: anchor.web3.Keypair,
  isBuy: boolean,
  amount: anchor.BN,
  price: anchor.BN,
  seq: number,
  expectedAfter: {
    bondingCurveRealSolReserves: anchor.BN;
    bondingCurveRealTokenReserves: anchor.BN;
    bondingCurveVirtualSolReserves: anchor.BN;
    bondingCurveVirtualTokenReserves: anchor.BN;
    bondingCurveTokenSupply: anchor.BN;
    traderTokenBalance: anchor.BN;
  }
) => {
  const { mint, bondingCurvePda, bondingCurveAta, marketData, marketVersion } =
    ctx;
  const { tradingFeeTreasury, tradingFeeBps } = marketData;
  const program = anchor.workspace.Tokens as Program<Tokens>;
  const bondingCurveSolBalanceBefore = await provider.connection.getBalance(
    bondingCurvePda
  );

  const payerSolBalanceBefore = await provider.connection.getBalance(
    payer.publicKey
  );
  const treasurySolBalanceBefore = await provider.connection.getBalance(
    tradingFeeTreasury
  );
  console.log(`[${seq}] Seller SOL Balance before: ${payerSolBalanceBefore}`);
  console.log(
    `[${seq}] Bonding Curve SOL Balance before: ${bondingCurveSolBalanceBefore}`
  );
  // Get buyer SOL balance before buying
  const method = await (async () => {
    const { ata: traderAta, preInstructions } = await withAtaInitInstruction(
      provider,
      mint.publicKey,
      payer
    );
    if (!isBuy) {
      return program.methods.sellTokens(amount, price, marketVersion).accounts({
        mint: mint.publicKey,
        seller: payer.publicKey,
        tradingFeeTreasury: ctx.marketData.tradingFeeTreasury,
      });
    }
    return program.methods
      .buyTokens(price, amount, marketVersion)
      .accounts({
        mint: mint.publicKey,
        buyer: payer.publicKey,
        tradingFeeTreasury: ctx.marketData.tradingFeeTreasury,
      })
      .preInstructions(preInstructions);
  })();
  const tx = await method.signers([payer]).rpc();
  console.log(`[${seq}] TX`, tx);
  const treasurySolBalanceAfter = await provider.connection.getBalance(
    tradingFeeTreasury
  );
  // ATA assertions
  const expectedBondingCurveRealTokenReserves =
    expectedAfter.bondingCurveRealTokenReserves;
  const tokenAccountR = await getAccount(provider.connection, bondingCurveAta);

  console.log(
    `[${seq}] Bonding Curve Balance: ${tokenAccountR.amount.toString()}. Expected: ${expectedBondingCurveRealTokenReserves.toString()}`
  );
  assert.equal(
    tokenAccountR.amount.toString(),
    expectedBondingCurveRealTokenReserves.toString(),
    'Bonding curve token balance should be updated'
  );
  // Bonding curve balance assertions
  const bondingCurveSolBalanceAfter = await provider.connection.getBalance(
    bondingCurvePda
  );
  console.log(
    `[${seq}] Bonding Curve SOL Balance Before: ${bondingCurveSolBalanceBefore}. After: ${bondingCurveSolBalanceAfter}`
  );
  // Bonding curve assertions
  const bondingCurveR = await program.account.bondingCurve.fetch(
    bondingCurvePda
  );
  const expectedBondingCurveRealSolReserves =
    expectedAfter.bondingCurveRealSolReserves;
  console.log(
    `[${seq}] SOL Reserves: ${bondingCurveR.realSolReserves.toString()}, Expected: ${expectedBondingCurveRealSolReserves.toString()}`
  );
  assert.equal(
    bondingCurveR.realSolReserves.toString(),
    expectedBondingCurveRealSolReserves.toString(),
    'SOL reserves should be updated'
  );
  console.log(
    `[${seq}] Token Reserves: ${bondingCurveR.realTokenReserves.toString()}, Expected: ${expectedBondingCurveRealTokenReserves.toString()}`
  );
  assert.equal(
    bondingCurveR.realTokenReserves.toString(),
    expectedBondingCurveRealTokenReserves.toString(),
    'Token reserves should be updated'
  );
  console.log(
    `[${seq}] Token Supply: ${bondingCurveR.tokenSupply.toString()}, Expected: ${expectedAfter.bondingCurveTokenSupply.toString()}`
  );
  assert.equal(
    bondingCurveR.tokenSupply.toString(),
    expectedAfter.bondingCurveTokenSupply.toString(),
    'Token supply should not be updated'
  );
  // Virtual reserves assertions
  console.log(
    `[${seq}] Virtual SOL Reserves: ${bondingCurveR.virtualSolReserves.toString()}, Expected: ${expectedAfter.bondingCurveVirtualSolReserves.toString()}`
  );
  assert.equal(
    bondingCurveR.virtualSolReserves.toString(),
    expectedAfter.bondingCurveVirtualSolReserves.toString(),
    'Virtual SOL reserves should be updated'
  );
  console.log(
    `[${seq}] Virtual Token Reserves: ${bondingCurveR.virtualTokenReserves.toString()}, Expected: ${expectedAfter.bondingCurveVirtualTokenReserves.toString()}`
  );
  assert.equal(
    bondingCurveR.virtualTokenReserves.toString(),
    expectedAfter.bondingCurveVirtualTokenReserves.toString(),
    'Virtual token reserves should be updated'
  );
  // Buyer assertions
  const payerSolBalanceAfter = await provider.connection.getBalance(
    payer.publicKey
  );
  console.log(
    `[${seq}] Payer Balance Before: ${payerSolBalanceBefore.toString()}, After: ${payerSolBalanceAfter.toString()}`
  );
  const payerSolBalanceIncrease = new anchor.BN(
    payerSolBalanceAfter.toString()
  ).sub(new anchor.BN(payerSolBalanceBefore.toString()));
  const payerSolBalanceDecrease = new anchor.BN(
    payerSolBalanceBefore.toString()
  ).sub(new anchor.BN(payerSolBalanceAfter.toString()));

  console.log(
    `[${seq}] Seller Balance: ${payerSolBalanceAfter.toString()}, Increased: ${payerSolBalanceIncrease.toString()}.`
  );

  const traderAta = getTraderAta(mint.publicKey, payer.publicKey);
  const traderAtaAccount = await getAccount(provider.connection, traderAta);
  const expectedTraderTokenBalance = expectedAfter.traderTokenBalance;
  console.log(
    `[${seq}] Payer Token Balance: ${traderAtaAccount.amount.toString()}, Expected: ${expectedTraderTokenBalance.toString()}`
  );
  assert.equal(
    traderAtaAccount.amount.toString(),
    expectedTraderTokenBalance.toString()
  );
  // Treasury assertions
  const treasurySolBalanceIncrease = new anchor.BN(
    treasurySolBalanceAfter.toString()
  ).sub(new anchor.BN(treasurySolBalanceBefore.toString()));
  const expectedFee = price
    .mul(new anchor.BN(tradingFeeBps))
    .divRound(new anchor.BN(10_000));
  console.log(
    `[${seq}] Fee Treasury Balance Before: ${treasurySolBalanceBefore.toString()}, After: ${treasurySolBalanceAfter.toString()}, Increase: ${treasurySolBalanceIncrease.toString()}, Expected Fee: ${expectedFee.toString()}`
  );
  assert.equal(
    treasurySolBalanceIncrease.toString(),
    expectedFee.toString(),
    'Treasury should have received the fee'
  );

  return {
    payerSolBalanceAfter,
    payerSolBalanceBefore,
    payerSolBalanceIncrease,
    payerSolBalanceDecrease,
    bondingCurveSolBalanceBefore,
    bondingCurveSolBalanceAfter,
    bondingCurveAccount: bondingCurveR,
  };
};

export const getTraderAta = (mint: PublicKey, trader: PublicKey) => {
  const ata = getAssociatedTokenAddressSync(mint, trader, true);
  console.log(`Trader ATA: ${ata}`);
  return ata;
};

export const airdropAndWait = async (
  provider: anchor.AnchorProvider,
  to: PublicKey,
  amount: number
) => {
  await provider.connection.requestAirdrop(to, amount);
  while (true) {
    const balance = await provider.connection.getBalance(to);
    if (balance > 0) break;
    console.log(`[2] No balance, waiting for it to settle...`);
    await sleep(500);
  }
};

export const ZERO_BN = new anchor.BN(0);

export const withAtaInitInstruction = async (
  provider: anchor.AnchorProvider,
  mint: PublicKey,
  payer: Keypair,
  authority = payer.publicKey,
  preInstructions: Array<TransactionInstruction> = []
) => {
  const program = anchor.workspace.Tokens as Program<Tokens>;
  const ata = getAssociatedTokenAddressSync(mint, authority, true);
  // TODO: check if it is already initialized
  const account = await provider.connection.getAccountInfo(ata);
  if (account) return { preInstructions, ata };

  const initAtaInst = await program.methods
    .initAta()
    .accounts({
      mint,
      payer: payer.publicKey,
      authority,
    })
    .signers([payer])
    .instruction();
  preInstructions.push(initAtaInst);
  return { preInstructions, ata };
};
