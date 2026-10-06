use anchor_lang::prelude::*;

use crate::{
    error::ErrorCode::{self},
    events::PolicyVersionCreated,
    ApprovalPolicyVersion, Member, Organization, MAX_POLICY_MEMBERS, MEMBER_SEED,
    ORGANIZATION_SEED, POLICY_SEED, ROLE_ADMIN, ROLE_APPROVER,
};

#[derive(Accounts)]
#[instruction(policy_id: u64, version: u64)]
pub struct CreatePolicyVersion<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [ORGANIZATION_SEED, organization.creator.as_ref(), organization.organization_id.to_le_bytes().as_ref()],
        bump = organization.bump,
    )]
    pub organization: Account<'info, Organization>,

    #[account(
        seeds = [MEMBER_SEED, organization.key().as_ref(), admin_member.member_id.to_le_bytes().as_ref()],
        bump = admin_member.bump,
        has_one = organization @ ErrorCode::MemberOrganizationMismatch,
        constraint = admin_member.active @ ErrorCode::InactiveMember,
        constraint = admin_member.authorized_wallet == authority.key() @ ErrorCode::UnauthorizedWallet,
        constraint = (admin_member.roles & ROLE_ADMIN) != 0 @ ErrorCode::MissingAdminRole

    )]
    pub admin_member: Account<'info, Member>,

    #[account(
        init,
        payer = authority,
        space = 8 + ApprovalPolicyVersion::INIT_SPACE,
        seeds = [
            POLICY_SEED,
            organization.key().as_ref(),
            policy_id.to_le_bytes().as_ref(),
            version.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub approval_policy_version: Account<'info, ApprovalPolicyVersion>,

    pub system_program: Program<'info, System>,
}

pub fn create_policy_version_handler(
    ctx: Context<CreatePolicyVersion>,
    policy_id: u64,
    version: u64,
    threshold: u8,
) -> Result<()> {
    require!(policy_id > 0, ErrorCode::InvalidPolicyId);
    require!(version > 0, ErrorCode::InvalidPolicyVersion);

    let eligible_count = ctx.remaining_accounts.len();

    require!(eligible_count > 0, ErrorCode::EmptyPolicyMembers);
    require!(
        eligible_count as u8 <= MAX_POLICY_MEMBERS,
        ErrorCode::TooManyPolicyMembers
    );

    require!(
        threshold > 0 && usize::from(threshold) <= eligible_count,
        ErrorCode::InvalidPolicyThreshold
    );

    let organization = ctx.accounts.organization.key();

    let mut eligible_members: Vec<Pubkey> = Vec::with_capacity(eligible_count);

    for account_info in ctx.remaining_accounts.iter() {
        let member_key = &account_info.key();

        // Reject the same Member account appearing more than once.
        require!(
            !eligible_members.contains(&member_key),
            ErrorCode::DuplicatePolicyMember
        );
        
        // check account owned by prgram
        require_eq!(
            *account_info.owner,
            crate::ID,
            ErrorCode::InvalidPolicyMember
        );

        /*
         * AccountInfo data is shared runtime memory stored behind
         * Rc<RefCell<...>>. try_borrow_data() obtains a checked immutable
         * borrow, after which we expose it as &[u8] for deserialization.
         */
        let account_data = account_info.try_borrow_data()?; // immutable
        let mut data: &[u8] = &account_data; // mutable

        let member = Member::try_deserialize(&mut data)
            .map_err(|_| error!(ErrorCode::InvalidPolicyMember))?;

        let (expected_member, expected_bump) = Pubkey::find_program_address(
            &[
                MEMBER_SEED,
                organization.as_ref(),
                member.member_id.to_le_bytes().as_ref(),
            ],
            ctx.program_id,
        );

        require_keys_eq!(expected_member, *member_key, ErrorCode::InvalidPolicyMember);

        require!(expected_bump == member.bump, ErrorCode::InvalidPolicyMember);

        require_keys_eq!(
            member.organization,
            organization,
            ErrorCode::MemberOrganizationMismatch
        );

        require!(member.active, ErrorCode::InactivePolicyMember);

        /*
         * This must be a bitmask check. An APPROVER | EXECUTOR Member is
         * still an eligible approver.
         */
        require!(
            (member.roles & ROLE_APPROVER) != 0,
            ErrorCode::PolicyMemberMissingApproverRole
        );

        eligible_members.push(*member_key);
    }

    let eligible_member_count = eligible_members.len() as u8;
    let policy_version = ctx.accounts.approval_policy_version.key();
    let created_by_member = ctx.accounts.admin_member.key();
    let created_by_wallet = ctx.accounts.authority.key();

    ctx.accounts
        .approval_policy_version
        .set_inner(ApprovalPolicyVersion {
            organization: ctx.accounts.organization.key(),
            policy_id,
            version,
            created_by_member,
            eligible_members,
            threshold,
            enabled: true,
            bump: ctx.bumps.approval_policy_version,
        });

    emit!(PolicyVersionCreated {
        organization,
        policy_version,
        policy_id,
        version,
        threshold,
        eligible_member_count,
        created_by_member,
        created_by_wallet,
    });

    Ok(())
}
