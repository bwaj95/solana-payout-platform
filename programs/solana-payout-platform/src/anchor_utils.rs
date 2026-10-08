use anchor_lang::prelude::*;
use anchor_spl::token::{self, TransferChecked};

pub fn transfer_tokens_checked_with_signer<'info>(
    authority: &AccountInfo<'info>,
    from: &AccountInfo<'info>,
    to: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    amount: u64,
    decimals: u8,
    signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    let accounts = TransferChecked {
        authority: authority.to_account_info(),
        from: from.to_account_info(),
        to: to.to_account_info(),
        mint: mint.to_account_info(),
    };

    let cpi_program = token_program.to_account_info();

    let cpi_ctx = CpiContext::new(*cpi_program.key, accounts).with_signer(signer_seeds);

    token::transfer_checked(cpi_ctx, amount, decimals)?;

    Ok(())
}
