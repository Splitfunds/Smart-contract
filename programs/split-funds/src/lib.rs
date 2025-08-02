use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer};

declare_id!("2JiRP2mrVXWBshpkif8F9e5vrvnHtJWUt5WUiyEftJKN");

#[program]
pub mod split_funds {
    use super::*;

     // Creates a new group for subscription splitting
    pub fn create_group(
        ctx: Context<CreateGroup>,
        group_name: String,
        total_cost: u64,
        subscription_due: i64,
    ) -> Result<()> {
        let group = &mut ctx.accounts.group;
        group.owner = *ctx.accounts.owner.key; // Group creator
        group.group_name = group_name;         // Name of the group
        group.total_cost = total_cost;         // Total subscription cost
        group.subscription_due = subscription_due; // Subscription due time (timestamp)
        group.member_count = 0;                // Initialize member count
        group.is_active = true;                // Mark group as active
        Ok(())
    }

    // Adds a new member to an existing group
    pub fn invite_member(ctx: Context<InviteMember>) -> Result<()> {
        let member = &mut ctx.accounts.member;
        member.group = ctx.accounts.group.key();
        member.member = *ctx.accounts.member_authority.key;
        member.contributed = 0;
        member.has_paid = false; // Mark as not paid
        Ok(())
    }

    // Allows a member to deposit their share into the escrow account
    pub fn deposit_funds(ctx: Context<DepositFunds>, amount: u64) -> Result<()> {
        let member = &mut ctx.accounts.member;
        let group = &mut ctx.accounts.group;
        let escrow = &mut ctx.accounts.escrow;

        // Ensure group is still active and user hasn't paid yet
        require!(group.is_active, CustomError::InactiveGroup);
        require!(!member.has_paid, CustomError::AlreadyPaid);

        // Transfer SPL tokens from member to escrow
        let cpi_accounts = Transfer {
            from: ctx.accounts.from_token_account.to_account_info(),
            to: ctx.accounts.escrow_token_account.to_account_info(),
            authority: ctx.accounts.member_authority.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Record contribution in member account
        member.contributed = amount;
        member.has_paid = true;
        escrow.total_held += amount;

        Ok(())
    }

    // Executes payout to the group owner after subscription due time
    pub fn execute_payout(ctx: Context<ExecutePayout>) -> Result<()> {
        let group = &mut ctx.accounts.group;
        let escrow = &mut ctx.accounts.escrow;

        // Ensure current time is past the subscription due time
        require!(Clock::get()?.unix_timestamp >= group.subscription_due, CustomError::TooEarly);

        let amount = escrow.total_held;

        // Use escrow account as signer via PDA
        let group_key = group.key();
        let seeds: &[&[u8]] = &[group_key.as_ref(), &[escrow.bump]];
        let signer = &[seeds];

        // Transfer SPL tokens from escrow to owner's token account
        let cpi_accounts = Transfer {
            from: ctx.accounts.escrow_token_account.to_account_info(),
            to: ctx.accounts.owner_token_account.to_account_info(),
            authority: ctx.accounts.escrow.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(ctx.accounts.token_program.to_account_info(), cpi_accounts, signer);
        token::transfer(cpi_ctx, amount)?;

        group.is_active = false; // Mark group as completed/inactive
        Ok(())
    }
}

// Context for creating a group
#[derive(Accounts)]
#[instruction(group_name: String)]
pub struct CreateGroup<'info> {
    #[account(init, payer = owner, space = 8 + 128)]
    pub group: Account<'info, GroupAccount>,
    #[account(mut)]
    pub owner: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Context for inviting a member
#[derive(Accounts)]
pub struct InviteMember<'info> {
    #[account(mut)]
    pub group: Account<'info, GroupAccount>,
    #[account(init, payer = member_authority, space = 8 + 64)]
    pub member: Account<'info, MemberAccount>,
    #[account(mut)]
    pub member_authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

// Context for depositing funds into the escrow
#[derive(Accounts)]
pub struct DepositFunds<'info> {
    #[account(mut)]
    pub group: Account<'info, GroupAccount>,
    #[account(mut)]
    pub member: Account<'info, MemberAccount>,
    #[account(mut)]
    pub member_authority: Signer<'info>,
    #[account(mut)]
    pub from_token_account: Account<'info, TokenAccount>, // Member's token account
    #[account(mut)]
    pub escrow_token_account: Account<'info, TokenAccount>, // Escrow's token account
    #[account(mut)]
    pub escrow: Account<'info, EscrowAccount>,
    pub token_program: Program<'info, Token>,
}

// Context for executing payout to group owner
#[derive(Accounts)]
pub struct ExecutePayout<'info> {
    #[account(mut)]
    pub group: Account<'info, GroupAccount>,
    #[account(mut)]
    pub escrow: Account<'info, EscrowAccount>,
    #[account(mut)]
    pub escrow_token_account: Account<'info, TokenAccount>, // Escrow's token account
    #[account(mut)]
    pub owner_token_account: Account<'info, TokenAccount>,  // Group owner's token account
    pub token_program: Program<'info, Token>,
}

// Group metadata and configuration
#[account]
pub struct GroupAccount {
    pub owner: Pubkey,
    pub group_name: String,
    pub total_cost: u64,
    pub subscription_due: i64,
    pub member_count: u8,
    pub is_active: bool,
}

// Individual member contributions
#[account]
pub struct MemberAccount {
    pub group: Pubkey,
    pub member: Pubkey,
    pub contributed: u64,
    pub has_paid: bool,
}

// Escrow account that holds SPL tokens until payout
#[account]
pub struct EscrowAccount {
    pub group: Pubkey,
    pub total_held: u64,
    pub bump: u8, // PDA bump seed
}

// Custom errors for better debugging and control
#[error_code]
pub enum CustomError {
    #[msg("Group is no longer active.")]
    InactiveGroup,
    #[msg("Member has already paid.")]
    AlreadyPaid,
    #[msg("Payout attempted before due time.")]
    TooEarly,
}

