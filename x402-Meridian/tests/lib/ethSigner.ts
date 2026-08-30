import { Secp256k1Program, TransactionInstruction } from "@solana/web3.js";
import { SigningKey, Wallet, keccak256 as ethersKeccak256, getBytes } from "ethers";

export class EthBuyer {
  readonly wallet: Wallet;

  constructor(privateKeyHex: string) {
    this.wallet = new Wallet(privateKeyHex);
  }

  get ethAddress(): Buffer {
    return Buffer.from(this.wallet.address.slice(2), "hex");
  }

  buildSecp256k1Instruction(dataHash: Buffer, instructionIndex: number): TransactionInstruction {
    if (dataHash.length !== 32) {
      throw new Error(`dataHash must be 32 bytes, got ${dataHash.length}`);
    }

    const digest = getBytes(ethersKeccak256(dataHash));
    const signingKey = new SigningKey(this.wallet.privateKey);
    const sig = signingKey.sign(digest);

    const signature = Buffer.concat([getBytes(sig.r), getBytes(sig.s)]);
    const recoveryId = sig.yParity;

    return Secp256k1Program.createInstructionWithEthAddress({
      ethAddress: this.ethAddress,
      message: dataHash,
      signature,
      recoveryId,
      instructionIndex,
    });
  }
}
