import * as anchor from '@coral-xyz/anchor';
import { Program } from '@coral-xyz/anchor';
import { SplitFunds } from '../target/types/split_funds';
import { assert } from 'chai';
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
} from '@solana/spl-token';

describe('split_funds', () => {
  // Configure Anchor provider
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.SplitFunds as Program<SplitFunds>;

  let groupAccount: anchor.web3.Keypair;
  let memberAccount: anchor.web3.Keypair;
  let escrowAccount: anchor.web3.Keypair;
  let mint: anchor.web3.PublicKey;
  let memberTokenAccount: anchor.web3.PublicKey;
  let escrowTokenAccount: anchor.web3.PublicKey;
  let ownerTokenAccount: anchor.web3.PublicKey;

  const owner = provider.wallet as anchor.Wallet;
  const member = anchor.web3.Keypair.generate();

  const groupName = 'Test Group';
  const totalCost = new anchor.BN(1000);
  const subscriptionDue = new anchor.BN(Math.floor(Date.now() / 1000) + 2); // 2 seconds in future

  before(async () => {
    // Create token mint
    mint = await createMint(
      provider.connection,
      owner.payer,
      owner.publicKey,
      null,
      0, // decimals
    );

    // Create token accounts
    memberTokenAccount = await createAccount(
      provider.connection,
      owner.payer,
      mint,
      member.publicKey,
    );
    escrowTokenAccount = await createAccount(
      provider.connection,
      owner.payer,
      mint,
      owner.publicKey,
    );
    ownerTokenAccount = await createAccount(
      provider.connection,
      owner.payer,
      mint,
      owner.publicKey,
    );

    // Mint some tokens to member
    await mintTo(
      provider.connection,
      owner.payer,
      mint,
      memberTokenAccount,
      owner.publicKey,
      1000,
    );

    // Generate PDAs/accounts
    groupAccount = anchor.web3.Keypair.generate();
    memberAccount = anchor.web3.Keypair.generate();
    escrowAccount = anchor.web3.Keypair.generate();

    // Fund the member so they can pay rent
    await provider.connection.requestAirdrop(
      member.publicKey,
      anchor.web3.LAMPORTS_PER_SOL,
    );
  });

  it('Creates a group', async () => {
    await program.methods
      .createGroup(groupName, totalCost, subscriptionDue)
      .accounts({
        group: groupAccount.publicKey,
        owner: owner.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      } as any)
      .signers([groupAccount])
      .rpc();

    const group = await program.account.groupAccount.fetch(
      groupAccount.publicKey,
    );
    assert.equal(group.groupName, groupName);
    assert.ok(group.isActive);
  });

  it('Invites a member', async () => {
    await program.methods
      .inviteMember()
      .accounts({
        group: groupAccount.publicKey,
        member: memberAccount.publicKey,
        memberAuthority: member.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      } as any)
      .signers([member, memberAccount])
      .rpc();

    const memberData = await program.account.memberAccount.fetch(
      memberAccount.publicKey,
    );
    assert.equal(memberData.hasPaid, false);
  });

  it('Member deposits funds', async () => {
    await program.methods
      .depositFunds(new anchor.BN(500))
      .accounts({
        group: groupAccount.publicKey,
        member: memberAccount.publicKey,
        memberAuthority: member.publicKey,
        fromTokenAccount: memberTokenAccount,
        escrowTokenAccount: escrowTokenAccount,
        escrow: escrowAccount.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .signers([member])
      .rpc();

    const memberData = await program.account.memberAccount.fetch(
      memberAccount.publicKey,
    );
    assert.equal(memberData.hasPaid, true);
  });

  it('Executes payout after due time', async () => {
    // Wait until subscription_due passes
    await new Promise((resolve) => setTimeout(resolve, 3000));

    await program.methods
      .executePayout()
      .accounts({
        group: groupAccount.publicKey,
        escrow: escrowAccount.publicKey,
        escrowTokenAccount: escrowTokenAccount,
        ownerTokenAccount: ownerTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      } as any)
      .rpc();

    const group = await program.account.groupAccount.fetch(
      groupAccount.publicKey,
    );
    assert.equal(group.isActive, false);
  });
});
