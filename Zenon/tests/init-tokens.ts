import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import { Tokens } from '../target/types/tokens';
import { deserializeMetadata } from '@metaplex-foundation/mpl-token-metadata';
import { getMint, getAccount } from '@solana/spl-token';
import { assert } from 'chai';

import { MOCK_TOKEN_METADATA_10_000, initializeRandomToken } from './helpers';

describe('Token Initialization', async () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const payer = (provider.wallet as anchor.Wallet).payer;
  const program = anchor.workspace.Tokens as Program<Tokens>;

  it('should initialize the token', async () => {
    const {
      mint,
      bondingCurveAta,
      bondingCurvePda,
      metadataAddress,
      marketData,
    } = await initializeRandomToken(program, payer, MOCK_TOKEN_METADATA_10_000);
    // Mint assertions
    const mintInfo = await getMint(provider.connection, mint.publicKey);
    assert.equal(mintInfo.mintAuthority, null, 'Mint authority should be null');
    assert.equal(mintInfo.supply.toString(), marketData.initialMint.toString());
    assert.equal(mintInfo.decimals, 6);
    assert.equal(mintInfo.freezeAuthority, null);
    // ATA assertions
    const tokenAccountR = await getAccount(
      provider.connection,
      bondingCurveAta
    );
    assert.equal(
      tokenAccountR.owner.toBase58(),
      bondingCurvePda.toBase58(),
      'ATA should be owned by bonding curve'
    );
    assert.equal(
      tokenAccountR.amount.toString(),
      marketData.initialMint.toString()
    );
    assert.equal(tokenAccountR.delegate, null);
    assert.equal(tokenAccountR.isFrozen, false);
    assert.equal(tokenAccountR.mint.toBase58(), mint.publicKey.toBase58());
    // Bonding curve assertions
    const bondingCurveR = await program.account.bondingCurve.fetch(
      bondingCurvePda
    );
    assert.equal(bondingCurveR.completed, false);
    assert.equal(bondingCurveR.realSolReserves.toString(), '0');
    assert.equal(
      bondingCurveR.realTokenReserves.toString(),
      marketData.initialMint.toString()
    );
    assert.equal(
      bondingCurveR.tokenSupply.toString(),
      marketData.initialMint.toString()
    );
    assert.equal(
      bondingCurveR.virtualSolReserves.toString(),
      MOCK_TOKEN_METADATA_10_000.solOffset.toString(),
      'Virtual Sol Reserves mismatch'
    );
    // Metadata assertions
    const metadataAccountInfo = await provider.connection.getAccountInfo(
      metadataAddress
    );
    const metadata = deserializeMetadata(metadataAccountInfo as any);
    assert.equal(metadata.uri, MOCK_TOKEN_METADATA_10_000.uri);
    assert.equal(metadata.name, MOCK_TOKEN_METADATA_10_000.name);
    assert.equal(metadata.symbol, MOCK_TOKEN_METADATA_10_000.symbol);
  });
});
