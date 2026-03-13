use anchor_lang::prelude::*;
use anchor_lang::system_program::{self, Transfer as SystemTransfer};
use anchor_spl::token::{self, Mint, Token, TokenAccount, Transfer as TokenTransfer};
use anchor_spl::associated_token::AssociatedToken;
use solana_security_txt::security_txt;
use std::str::FromStr;

security_txt! {
    name: "DumpLock",
    project_url: "https://dumplock.io",
    contacts: "mailto:security@dumplock.io",
    policy: "https://dumplock.io/security",
    preferred_languages: "en",
    source_code: "https://github.com/IDX-SOL/dumplock-program",
    auditors: "None",
    acknowledgements: "https://dumplock.io"
}

declare_id!("GoB3WxFTNYh8kqicpC1eYjnJCx9qpcy2TuGsVqB38pmz");

const TREASURY_WALLET: &str = "DPByYJaAF7vxiBUCj3JcV58EZYN539xEMr1hJzUrdc7s";
const LOCK_FEE_LAMPORTS: u64 = 50_000_000;

#[program]
pub mod onchain_dumplock {
    use super::*;

    pub fn lock(
        ctx: Context<Lock>,
        lock_percent: u8,
        lock_duration_hours: u64,
    ) -> Result<()> {
        let treasury_wallet = Pubkey::from_str(TREASURY_WALLET)
            .map_err(|_| error!(DumpLockError::InvalidTreasury))?;

        require_keys_eq!(
            ctx.accounts.treasury.key(),
            treasury_wallet,
            DumpLockError::InvalidTreasury
        );

        require!(
            lock_percent == 95 || lock_percent == 97 || lock_percent == 99,
            DumpLockError::InvalidLockPercent
        );

        require!(
            lock_duration_hours == 6
                || lock_duration_hours == 12
                || lock_duration_hours == 24,
            DumpLockError::InvalidLockDuration
        );

        let mint = &ctx.accounts.mint;
        let creator_ata = &ctx.accounts.creator_ata;

        let total_supply = mint.supply;

        // Use u128 for the intermediate multiplication so large 9-decimal token
        // supplies do not overflow before dividing by 100.
        let locked_amount_u128 = u128::from(total_supply)
            .checked_mul(u128::from(lock_percent))
            .ok_or(DumpLockError::MathOverflow)?
            .checked_div(100)
            .ok_or(DumpLockError::MathOverflow)?;

        let locked_amount = u64::try_from(locked_amount_u128)
            .map_err(|_| error!(DumpLockError::MathOverflow))?;

        require!(
            creator_ata.amount >= locked_amount,
            DumpLockError::InsufficientBalance
        );

        let mint_authority_was_active = mint.mint_authority.is_some();
        let freeze_authority_was_active = mint.freeze_authority.is_some();

        let clock = Clock::get()?;
        let current_timestamp = clock.unix_timestamp;

        let duration_seconds = (lock_duration_hours as i64)
            .checked_mul(3600)
            .ok_or(DumpLockError::MathOverflow)?;

        let unlock_timestamp = current_timestamp
            .checked_add(duration_seconds)
            .ok_or(DumpLockError::MathOverflow)?;

        let (_pda, bump) = Pubkey::find_program_address(
            &[b"lock", mint.key().as_ref()],
            ctx.program_id,
        );

        let state = &mut ctx.accounts.lock_state;

        require!(
            state.locked_amount == 0 && !state.is_unlocked,
            DumpLockError::LockAlreadyUsed
        );

        state.creator = ctx.accounts.creator.key();
        state.mint = mint.key();
        state.locked_amount = locked_amount;
        state.unlock_timestamp = unlock_timestamp;
        state.created_timestamp = current_timestamp;
        state.lock_percent = lock_percent;
        state.bump = bump;
        state.is_unlocked = false;
        state.total_supply_at_lock = total_supply;
        state.mint_authority_was_active = mint_authority_was_active;
        state.freeze_authority_was_active = freeze_authority_was_active;

        let fee_accounts = SystemTransfer {
            from: ctx.accounts.creator.to_account_info(),
            to: ctx.accounts.treasury.to_account_info(),
        };

        let fee_ctx = CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            fee_accounts,
        );

        system_program::transfer(fee_ctx, LOCK_FEE_LAMPORTS)?;

        let cpi_accounts = TokenTransfer {
            from: creator_ata.to_account_info(),
            to: ctx.accounts.vault_ata.to_account_info(),
            authority: ctx.accounts.creator.to_account_info(),
        };

        let cpi_ctx =
            CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);

        token::transfer(cpi_ctx, locked_amount)?;

        Ok(())
    }

    pub fn extend(
        ctx: Context<Extend>,
        duration_hours: u64,
    ) -> Result<()> {

        require!(
            duration_hours == 6
                || duration_hours == 12
                || duration_hours == 24,
            DumpLockError::InvalidLockDuration
        );

        let clock = Clock::get()?;
        let state = &mut ctx.accounts.lock_state;

        require!(!state.is_unlocked, DumpLockError::AlreadyUnlocked);

        require!(
            clock.unix_timestamp < state.unlock_timestamp,
            DumpLockError::AlreadyUnlocked
        );

        let duration_seconds = (duration_hours as i64)
            .checked_mul(3600)
            .ok_or(DumpLockError::MathOverflow)?;

        state.unlock_timestamp = state
            .unlock_timestamp
            .checked_add(duration_seconds)
            .ok_or(DumpLockError::MathOverflow)?;

        Ok(())
    }

    pub fn unlock(ctx: Context<Unlock>) -> Result<()> {
        let clock = Clock::get()?;

        let state = &ctx.accounts.lock_state;

        require!(!state.is_unlocked, DumpLockError::AlreadyUnlocked);

        require!(
            clock.unix_timestamp >= state.unlock_timestamp,
            DumpLockError::LockStillActive
        );

        let locked_amount = state.locked_amount;
        let bump = state.bump;
        let mint = state.mint;

        let seeds = &[b"lock", mint.as_ref(), &[bump]];
        let signer = &[&seeds[..]];

        let cpi_accounts = TokenTransfer {
            from: ctx.accounts.vault_ata.to_account_info(),
            to: ctx.accounts.creator_ata.to_account_info(),
            authority: ctx.accounts.lock_state.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );

        token::transfer(cpi_ctx, locked_amount)?;

        let state_mut = &mut ctx.accounts.lock_state;
        state_mut.is_unlocked = true;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Lock<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        constraint = creator_ata.owner == creator.key(),
        constraint = creator_ata.mint == mint.key()
    )]
    pub creator_ata: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = creator,
        seeds = [b"lock", mint.key().as_ref()],
        bump,
        space = 8 + LockState::SIZE
    )]
    pub lock_state: Account<'info, LockState>,

    #[account(
        init,
        payer = creator,
        associated_token::mint = mint,
        associated_token::authority = lock_state
    )]
    pub vault_ata: Account<'info, TokenAccount>,

    #[account(mut)]
    pub treasury: SystemAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct Extend<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [b"lock", mint.key().as_ref()],
        bump = lock_state.bump,
        has_one = creator,
        has_one = mint
    )]
    pub lock_state: Account<'info, LockState>,
}

#[derive(Accounts)]
pub struct Unlock<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(
        mut,
        constraint = creator_ata.owner == creator.key(),
        constraint = creator_ata.mint == mint.key()
    )]
    pub creator_ata: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"lock", mint.key().as_ref()],
        bump = lock_state.bump,
        has_one = creator,
        has_one = mint
    )]
    pub lock_state: Account<'info, LockState>,
    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = lock_state
    )]
    pub vault_ata: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}

#[account]
pub struct LockState {
    pub creator: Pubkey,
    pub mint: Pubkey,
    pub locked_amount: u64,
    pub unlock_timestamp: i64,
    pub created_timestamp: i64,
    pub lock_percent: u8,
    pub bump: u8,
    pub is_unlocked: bool,
    pub total_supply_at_lock: u64,
    pub mint_authority_was_active: bool,
    pub freeze_authority_was_active: bool,
}

impl LockState {
    pub const SIZE: usize =
        32 + 32 + 8 + 8 + 8 + 1 + 1 + 1 + 8 + 1 + 1;
}

#[error_code]
pub enum DumpLockError {
    #[msg("Invalid lock percentage")]
    InvalidLockPercent,
    #[msg("Invalid lock duration")]
    InvalidLockDuration,
    #[msg("Insufficient token balance")]
    InsufficientBalance,
    #[msg("Lock is still active")]
    LockStillActive,
    #[msg("Tokens already unlocked")]
    AlreadyUnlocked,
    #[msg("This mint has already been locked and cannot be reused")]
    LockAlreadyUsed,
    #[msg("Invalid treasury account")]
    InvalidTreasury,
    #[msg("Math overflow detected")]
    MathOverflow,
}