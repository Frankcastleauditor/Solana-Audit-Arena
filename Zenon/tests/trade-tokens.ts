import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import { Tokens } from '../target/types/tokens';
import { Keypair } from '@solana/web3.js';
import { assert } from 'chai';

import {
  MOCK_TOKEN_METADATA_10_000,
  TRADING_CONFIG_10_000,
  ONE_TRADER_TRADING_CONFIG_10_000,
  ZERO_BN,
  airdropAndWait,
  initializeRandomToken,
  tradeAndAssert,
} from './helpers';

describe('Trade Tokens', async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const trader = (provider.wallet as anchor.Wallet).payer;
  const program = anchor.workspace.Tokens as Program<Tokens>;
  console.log(`1st trader: ${trader.publicKey}`);

  it('Buy and sell tokens', async () => {
    const ctx = await initializeRandomToken(
      program,
      trader,
      MOCK_TOKEN_METADATA_10_000
    );
    const { marketData } = ctx;
    // Buy the first block of tokens
    const expectedAfterFirstBuy = {
      bondingCurveRealSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE,
      bondingCurveRealTokenReserves: marketData.initialMint.sub(
        TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT
      ),
      bondingCurveTokenSupply: marketData.initialMint,
      traderTokenBalance: TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT,
      bondingCurveVirtualSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
        TRADING_CONFIG_10_000.SOL_OFFSET
      ),
      bondingCurveVirtualTokenReserves: marketData.initialMint
        .add(MOCK_TOKEN_METADATA_10_000.tokenOffset)
        .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT),
    };
    const buy1stTokens = await tradeAndAssert(
      provider,
      ctx,
      trader,
      true,
      TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT,
      TRADING_CONFIG_10_000.FIRST_BUY_PRICE,
      1,
      expectedAfterFirstBuy
    );

    const { payerSolBalanceDecrease: payerSolBalanceDecrease1 } = buy1stTokens;
    assert.equal(
      payerSolBalanceDecrease1.gte(TRADING_CONFIG_10_000.FIRST_BUY_PRICE),
      true,
      'Buyer SOL balance should have been decreased more than the price of the tokens'
    );
    // Buy the second block of tokens
    const secondTrader = Keypair.generate();
    await airdropAndWait(provider, secondTrader.publicKey, 100000000000);
    const bondingCurveRealSolReserves =
      TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
        TRADING_CONFIG_10_000.SECOND_BUY_PRICE
      );
    const bondingCurveRealTokenReserves = marketData.initialMint
      .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
      .sub(TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT);
    const expectedAfterSecondBuy = {
      bondingCurveRealSolReserves,
      bondingCurveRealTokenReserves,
      bondingCurveTokenSupply: marketData.initialMint,
      bondingCurveVirtualSolReserves: bondingCurveRealSolReserves.add(
        TRADING_CONFIG_10_000.SOL_OFFSET
      ),
      bondingCurveVirtualTokenReserves: bondingCurveRealTokenReserves.add(
        TRADING_CONFIG_10_000.TOKENS_OFFSET
      ),
      traderTokenBalance: TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT,
    };
    const buy2ndTokens = await tradeAndAssert(
      provider,
      ctx,
      secondTrader,
      true,
      TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT,
      TRADING_CONFIG_10_000.SECOND_BUY_PRICE,
      2,
      expectedAfterSecondBuy
    );
    const { payerSolBalanceDecrease: payerSolBalanceDecrease2 } = buy2ndTokens;
    assert.equal(
      payerSolBalanceDecrease2.gt(TRADING_CONFIG_10_000.SECOND_BUY_PRICE),
      true,
      'Buyer SOL balance should have been decreased more than the price of the tokens'
    );

    // Sell the second block of tokens
    const expectedAfterFirstSell = {
      bondingCurveRealSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
        TRADING_CONFIG_10_000.SECOND_BUY_PRICE
      ).sub(TRADING_CONFIG_10_000.FIRST_SELL_PRICE),
      bondingCurveRealTokenReserves: marketData.initialMint
        .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
        .sub(TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT)
        .add(TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT),
      bondingCurveTokenSupply: marketData.initialMint,
      bondingCurveVirtualSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
        TRADING_CONFIG_10_000.SECOND_BUY_PRICE
      )
        .sub(TRADING_CONFIG_10_000.FIRST_SELL_PRICE)
        .add(TRADING_CONFIG_10_000.SOL_OFFSET),
      bondingCurveVirtualTokenReserves: marketData.initialMint
        .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
        .sub(TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT)
        .add(TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT)
        .add(TRADING_CONFIG_10_000.TOKENS_OFFSET),
      traderTokenBalance: TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT.sub(
        TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT
      ),
    };
    const sell2ndTokens = await tradeAndAssert(
      provider,
      ctx,
      trader,
      false,
      TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT,
      TRADING_CONFIG_10_000.FIRST_SELL_PRICE,
      3,
      expectedAfterFirstSell
    );
    const { payerSolBalanceIncrease: payerSolBalanceIncrease2 } = sell2ndTokens;
    assert.equal(
      payerSolBalanceIncrease2.gte(ZERO_BN),
      true,
      'Seller SOL balance should have been increased'
    );
    // Sell the first block of tokens
    const expectedAfterSecondSell = {
      bondingCurveRealSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
        TRADING_CONFIG_10_000.SECOND_BUY_PRICE
      )
        .sub(TRADING_CONFIG_10_000.FIRST_SELL_PRICE)
        .sub(TRADING_CONFIG_10_000.SECOND_SELL_PRICE),
      bondingCurveRealTokenReserves: marketData.initialMint
        .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
        .sub(TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT)
        .add(TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT)
        .add(TRADING_CONFIG_10_000.SECOND_SELL_AMOUNT),
      bondingCurveTokenSupply: marketData.initialMint,
      bondingCurveVirtualSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
        TRADING_CONFIG_10_000.SECOND_BUY_PRICE
      )
        .sub(TRADING_CONFIG_10_000.FIRST_SELL_PRICE)
        .sub(TRADING_CONFIG_10_000.SECOND_SELL_PRICE)
        .add(TRADING_CONFIG_10_000.SOL_OFFSET),
      bondingCurveVirtualTokenReserves: marketData.initialMint
        .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
        .sub(TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT)
        .add(TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT)
        .add(TRADING_CONFIG_10_000.SECOND_SELL_AMOUNT)
        .add(TRADING_CONFIG_10_000.TOKENS_OFFSET),
      traderTokenBalance: TRADING_CONFIG_10_000.SECOND_BUY_AMOUNT.sub(
        TRADING_CONFIG_10_000.SECOND_SELL_AMOUNT
      ),
    };
    const sell1stTokens = await tradeAndAssert(
      provider,
      ctx,
      secondTrader,
      false,
      TRADING_CONFIG_10_000.SECOND_SELL_AMOUNT,
      TRADING_CONFIG_10_000.SECOND_SELL_PRICE,
      4,
      expectedAfterSecondSell
    );
    const { payerSolBalanceIncrease: payerSolBalanceIncrease1 } = sell1stTokens;
    assert.equal(
      payerSolBalanceIncrease1.gte(ZERO_BN),
      true,
      'Seller SOL balance should have been increased'
    );
  });

  it('Validate minimum tokens amount', async () => {
    const ctx = await initializeRandomToken(
      program,
      trader,
      MOCK_TOKEN_METADATA_10_000
    );
    const { marketData } = ctx;
    // Buy the first block of tokens
    const minBuyAmount = TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT.addn(1);
    try {
      const buy1stTokens = await tradeAndAssert(
        provider,
        ctx,
        trader,
        true,
        minBuyAmount,
        TRADING_CONFIG_10_000.FIRST_BUY_PRICE,
        10,
        {
          bondingCurveRealSolReserves: TRADING_CONFIG_10_000.FIRST_BUY_PRICE,
          bondingCurveRealTokenReserves: marketData.initialMint.sub(
            TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT
          ),
          bondingCurveTokenSupply: marketData.initialMint,
          traderTokenBalance: TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT,
          bondingCurveVirtualSolReserves:
            TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
              TRADING_CONFIG_10_000.SOL_OFFSET
            ),
          bondingCurveVirtualTokenReserves: marketData.initialMint
            .add(MOCK_TOKEN_METADATA_10_000.tokenOffset)
            .sub(TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT),
        }
      );
      assert.fail('Should have failed');
    } catch (err) {
      assert.equal(
        'MinTokenAmountNotMet',
        err.error?.errorCode?.code,
        'MinTokenAmountNotMet error when buying'
      );
    }
  });

  it('Validate min sol amount when selling', async () => {
    const ctx = await initializeRandomToken(
      program,
      trader,
      MOCK_TOKEN_METADATA_10_000
    );
    const { marketData } = ctx;
    // Buy the first block of tokens
    const expectedAfterFirstBuy = {
      bondingCurveRealSolReserves:
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_PRICE,
      bondingCurveRealTokenReserves: marketData.initialMint.sub(
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT
      ),
      bondingCurveTokenSupply: marketData.initialMint,
      traderTokenBalance: ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT,
      bondingCurveVirtualSolReserves:
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_PRICE.add(
          MOCK_TOKEN_METADATA_10_000.solOffset
        ),
      bondingCurveVirtualTokenReserves: marketData.initialMint
        .add(MOCK_TOKEN_METADATA_10_000.tokenOffset)
        .sub(ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT),
    };
    const buy1stTokens = await tradeAndAssert(
      provider,
      ctx,
      trader,
      true,
      ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT,
      ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_PRICE,
      20,
      expectedAfterFirstBuy
    );

    const { payerSolBalanceDecrease: payerSolBalanceDecrease1 } = buy1stTokens;
    assert.equal(
      payerSolBalanceDecrease1.gte(
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_PRICE
      ),
      true,
      'Buyer SOL balance should have been decreased more than the price of the tokens'
    );

    // Sell the second block of tokens
    const expectedAfterFirstSell = {
      bondingCurveRealSolReserves:
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_PRICE.sub(
          ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_PRICE
        ),
      bondingCurveRealTokenReserves: marketData.initialMint
        .sub(ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
        .add(ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT),
      bondingCurveTokenSupply: marketData.initialMint,
      bondingCurveVirtualSolReserves:
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_PRICE.sub(
          ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_PRICE
        ).add(MOCK_TOKEN_METADATA_10_000.solOffset),
      bondingCurveVirtualTokenReserves: marketData.initialMint
        .sub(ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT)
        .add(ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT)
        .add(ONE_TRADER_TRADING_CONFIG_10_000.TOKENS_OFFSET),
      traderTokenBalance: ONE_TRADER_TRADING_CONFIG_10_000.FIRST_BUY_AMOUNT.sub(
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT
      ),
    };
    try {
      const sell2ndTokens = await tradeAndAssert(
        provider,
        ctx,
        trader,
        false,
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_AMOUNT,
        ONE_TRADER_TRADING_CONFIG_10_000.FIRST_SELL_PRICE.addn(1),
        21,
        expectedAfterFirstSell
      );
      assert.fail('Should have failed');
    } catch (err) {
      assert.equal(
        'MinSolAmountNotMet',
        err.error?.errorCode?.code,
        'MinSolAmountNotMet error when selling'
      );
    }
  });
});
