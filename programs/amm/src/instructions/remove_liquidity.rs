use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn};
use num_traits::ToPrimitive;

use crate::*;
use crate::utils::token_transfer_signed;

pub fn handler(ctx: Context<AddOrRemoveLiquidity>, withdraw_bps: u64) -> Result<()> {
    let AddOrRemoveLiquidity {
        user,
        amm,
        amm_position,
        lp_mint,

        base_mint,
        quote_mint,
        user_ata_lp,
        user_ata_base,
        user_ata_quote,
        vault_ata_base,
        vault_ata_quote,
        associated_token_program: _,
        token_program,
        system_program: _,
    } = ctx.accounts;

    assert!(amm_position.ownership > 0);
    assert!(withdraw_bps > 0);
    assert!(withdraw_bps <= BPS_SCALE);

    amm.update_twap(Clock::get()?.slot);

    let base_to_withdraw = (amm.base_amount as u128)
        .checked_mul(amm_position.ownership as u128)
        .unwrap()
        .checked_mul(withdraw_bps as u128)
        .unwrap()
        .checked_div(BPS_SCALE as u128)
        .unwrap()
        .checked_div(amm.total_ownership as u128)
        .unwrap()
        .to_u64()
        .unwrap();

    let quote_to_withdraw = (amm.quote_amount as u128)
        .checked_mul(amm_position.ownership as u128)
        .unwrap()
        .checked_mul(withdraw_bps as u128)
        .unwrap()
        .checked_div(BPS_SCALE as u128)
        .unwrap()
        .checked_div(amm.total_ownership as u128)
        .unwrap()
        .to_u64()
        .unwrap();

    // for rounding up, if we have, a = b / c, we use: a = (b + (c - 1)) / c
    let less_ownership = (amm_position.ownership as u128)
        .checked_mul(withdraw_bps as u128)
        .unwrap()
        .checked_add(BPS_SCALE as u128 - 1)
        .unwrap()
        .checked_div(BPS_SCALE as u128)
        .unwrap()
        .to_u64()
        .unwrap();

    amm_position.ownership = amm_position.ownership.checked_sub(less_ownership).unwrap();
    amm.total_ownership = amm.total_ownership.checked_sub(less_ownership).unwrap();

    token::burn(
        CpiContext::new(
            token_program.to_account_info(),
            Burn {
                mint: lp_mint.to_account_info(),
                from: user_ata_lp.to_account_info(),
                authority: user.to_account_info(),
            },
        ),
        less_ownership,
    )?;

    amm.base_amount = amm.base_amount.checked_sub(base_to_withdraw).unwrap();
    amm.quote_amount = amm.quote_amount.checked_sub(quote_to_withdraw).unwrap();

    let base_mint_key = base_mint.key();
    let quote_mint_key = quote_mint.key();

    let seeds = generate_vault_seeds!(base_mint_key, quote_mint_key, amm.bump);

    // send vault base tokens to user
    token_transfer_signed(
        base_to_withdraw,
        token_program,
        vault_ata_base,
        user_ata_base,
        amm,
        seeds,
    )?;

    // send vault quote tokens to user
    token_transfer_signed(
        quote_to_withdraw,
        token_program,
        vault_ata_quote,
        user_ata_quote,
        amm,
        seeds,
    )?;

    Ok(())
}
