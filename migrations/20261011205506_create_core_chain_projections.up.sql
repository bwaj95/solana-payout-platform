-- This additional key allows child chain projections to reference not only
-- the application organization ID, but the exact Organization PDA observed
-- for that cluster and program deployment.
ALTER TABLE organization_chain_accounts
ADD CONSTRAINT organization_chain_accounts_identity_with_pda_unique
UNIQUE (
    organization_id,
    cluster,
    program_id,
    organization_pda
);


CREATE TABLE member_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    member_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    member_pda TEXT NOT NULL,

    authorized_wallet TEXT NOT NULL,
    authorization_revision NUMERIC(20, 0) NOT NULL,
    roles INTEGER NOT NULL,
    active BOOLEAN NOT NULL,
    bump SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT member_chain_accounts_pkey
        PRIMARY KEY (
            organization_id,
            member_id,
            cluster,
            program_id
        ),

    CONSTRAINT member_chain_accounts_application_member_fkey
        FOREIGN KEY (
            organization_id,
            member_id
        )
        REFERENCES organization_members (
            organization_id,
            member_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT member_chain_accounts_organization_chain_fkey
        FOREIGN KEY (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        REFERENCES organization_chain_accounts (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT member_chain_accounts_address_unique
        UNIQUE (
            cluster,
            program_id,
            member_pda
        ),

    -- Enables MemberWallet and policy-member rows to reference both the
    -- numeric application member identity and its exact Member PDA.
    CONSTRAINT member_chain_accounts_identity_with_pda_unique
        UNIQUE (
            organization_id,
            member_id,
            cluster,
            program_id,
            member_pda
        ),

    CONSTRAINT member_chain_accounts_authorization_revision_u64_range
        CHECK (
            authorization_revision >= 0
            AND authorization_revision <= 18446744073709551615
        ),

    CONSTRAINT member_chain_accounts_roles_u16_range
        CHECK (
            roles >= 0
            AND roles <= 65535
        ),

    CONSTRAINT member_chain_accounts_wallet_length
        CHECK (
            char_length(authorized_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT member_chain_accounts_pda_length
        CHECK (
            char_length(member_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT member_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT member_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT member_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);

CREATE INDEX member_chain_accounts_wallet_index
    ON member_chain_accounts (
        organization_id,
        authorized_wallet
    );


-- MemberWallet has no numeric application ID. Its identity comes from:
-- [MEMBER_WALLET_SEED, organization_pda, authorized_wallet].
CREATE TABLE member_wallet_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    member_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    member_pda TEXT NOT NULL,
    member_wallet_pda TEXT NOT NULL,
    authorized_wallet TEXT NOT NULL,

    bump SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT member_wallet_chain_accounts_pkey
        PRIMARY KEY (
            cluster,
            program_id,
            member_wallet_pda
        ),

    CONSTRAINT member_wallet_chain_accounts_member_chain_fkey
        FOREIGN KEY (
            organization_id,
            member_id,
            cluster,
            program_id,
            member_pda
        )
        REFERENCES member_chain_accounts (
            organization_id,
            member_id,
            cluster,
            program_id,
            member_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT member_wallet_chain_accounts_seed_identity_unique
        UNIQUE (
            organization_id,
            cluster,
            program_id,
            authorized_wallet
        ),

    CONSTRAINT member_wallet_chain_accounts_pda_length
        CHECK (
            char_length(member_wallet_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT member_wallet_chain_accounts_wallet_length
        CHECK (
            char_length(authorized_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT member_wallet_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT member_wallet_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT member_wallet_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);


CREATE TABLE recipient_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    recipient_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    recipient_pda TEXT NOT NULL,

    current_destination TEXT NOT NULL,
    wallet_revision BIGINT NOT NULL,
    active BOOLEAN NOT NULL,
    bump SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT recipient_chain_accounts_pkey
        PRIMARY KEY (
            organization_id,
            recipient_id,
            cluster,
            program_id
        ),

    CONSTRAINT recipient_chain_accounts_application_recipient_fkey
        FOREIGN KEY (
            organization_id,
            recipient_id
        )
        REFERENCES recipients (
            organization_id,
            recipient_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT recipient_chain_accounts_organization_chain_fkey
        FOREIGN KEY (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        REFERENCES organization_chain_accounts (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT recipient_chain_accounts_address_unique
        UNIQUE (
            cluster,
            program_id,
            recipient_pda
        ),

    CONSTRAINT recipient_chain_accounts_identity_with_pda_unique
        UNIQUE (
            organization_id,
            recipient_id,
            cluster,
            program_id,
            recipient_pda
        ),

    CONSTRAINT recipient_chain_accounts_wallet_revision_u32_range
        CHECK (
            wallet_revision >= 0
            AND wallet_revision <= 4294967295
        ),

    CONSTRAINT recipient_chain_accounts_destination_length
        CHECK (
            char_length(current_destination) BETWEEN 32 AND 44
        ),

    CONSTRAINT recipient_chain_accounts_pda_length
        CHECK (
            char_length(recipient_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT recipient_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT recipient_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT recipient_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);


CREATE TABLE vault_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    vault_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    vault_pda TEXT NOT NULL,

    asset_kind TEXT NOT NULL,
    mint_address TEXT,

    reserved_total NUMERIC(20, 0) NOT NULL,
    active BOOLEAN NOT NULL,
    bump SMALLINT NOT NULL,

    -- The token account and balance are derived observations. They are not
    -- fields serialized inside VaultState, but are required by the product
    -- to display available funds.
    vault_token_account TEXT,
    token_balance NUMERIC(20, 0),
    token_balance_observed_slot BIGINT,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT vault_chain_accounts_pkey
        PRIMARY KEY (
            organization_id,
            vault_id,
            cluster,
            program_id
        ),

    CONSTRAINT vault_chain_accounts_application_vault_fkey
        FOREIGN KEY (
            organization_id,
            vault_id
        )
        REFERENCES vaults (
            organization_id,
            vault_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT vault_chain_accounts_organization_chain_fkey
        FOREIGN KEY (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        REFERENCES organization_chain_accounts (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT vault_chain_accounts_address_unique
        UNIQUE (
            cluster,
            program_id,
            vault_pda
        ),

    CONSTRAINT vault_chain_accounts_identity_with_pda_unique
        UNIQUE (
            organization_id,
            vault_id,
            cluster,
            program_id,
            vault_pda
        ),

    CONSTRAINT vault_chain_accounts_asset_kind_valid
        CHECK (
            asset_kind IN ('native_sol', 'spl')
        ),

    CONSTRAINT vault_chain_accounts_asset_configuration_valid
        CHECK (
            (
                asset_kind = 'native_sol'
                AND mint_address IS NULL
                AND vault_token_account IS NULL
            )
            OR
            (
                asset_kind = 'spl'
                AND mint_address IS NOT NULL
                AND char_length(mint_address) BETWEEN 32 AND 44
                AND vault_token_account IS NOT NULL
                AND char_length(vault_token_account) BETWEEN 32 AND 44
            )
        ),

    CONSTRAINT vault_chain_accounts_reserved_total_u64_range
        CHECK (
            reserved_total >= 0
            AND reserved_total <= 18446744073709551615
        ),

    CONSTRAINT vault_chain_accounts_token_balance_u64_range
        CHECK (
            token_balance IS NULL
            OR (
                token_balance >= 0
                AND token_balance <= 18446744073709551615
            )
        ),

    CONSTRAINT vault_chain_accounts_token_balance_slot_valid
        CHECK (
            token_balance_observed_slot IS NULL
            OR token_balance_observed_slot >= 0
        ),

    CONSTRAINT vault_chain_accounts_pda_length
        CHECK (
            char_length(vault_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT vault_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT vault_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT vault_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);


CREATE TABLE approval_policy_version_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    policy_id NUMERIC(20, 0) NOT NULL,
    version NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    policy_version_pda TEXT NOT NULL,

    created_by_member_id NUMERIC(20, 0) NOT NULL,
    created_by_member_pda TEXT NOT NULL,

    threshold SMALLINT NOT NULL,
    enabled BOOLEAN NOT NULL,
    bump SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT approval_policy_version_chain_accounts_pkey
        PRIMARY KEY (
            organization_id,
            policy_id,
            version,
            cluster,
            program_id
        ),

    CONSTRAINT approval_policy_version_chain_accounts_application_version_fkey
        FOREIGN KEY (
            organization_id,
            policy_id,
            version
        )
        REFERENCES approval_policy_versions (
            organization_id,
            policy_id,
            version
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_version_chain_accounts_organization_chain_fkey
        FOREIGN KEY (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        REFERENCES organization_chain_accounts (
            organization_id,
            cluster,
            program_id,
            organization_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_version_chain_accounts_creator_chain_fkey
        FOREIGN KEY (
            organization_id,
            created_by_member_id,
            cluster,
            program_id,
            created_by_member_pda
        )
        REFERENCES member_chain_accounts (
            organization_id,
            member_id,
            cluster,
            program_id,
            member_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_version_chain_accounts_address_unique
        UNIQUE (
            cluster,
            program_id,
            policy_version_pda
        ),

    CONSTRAINT approval_policy_version_chain_accounts_identity_with_pda_unique
        UNIQUE (
            organization_id,
            policy_id,
            version,
            cluster,
            program_id,
            policy_version_pda
        ),

    CONSTRAINT approval_policy_version_chain_accounts_threshold_range
        CHECK (
            threshold BETWEEN 1 AND 8
        ),

    CONSTRAINT approval_policy_version_chain_accounts_pda_length
        CHECK (
            char_length(policy_version_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT approval_policy_version_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT approval_policy_version_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT approval_policy_version_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);


CREATE TABLE approval_policy_version_chain_members (
    organization_id NUMERIC(20, 0) NOT NULL,
    policy_id NUMERIC(20, 0) NOT NULL,
    version NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    policy_version_pda TEXT NOT NULL,

    member_id NUMERIC(20, 0) NOT NULL,
    member_pda TEXT NOT NULL,
    member_order SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT approval_policy_version_chain_members_pkey
        PRIMARY KEY (
            organization_id,
            policy_id,
            version,
            cluster,
            program_id,
            member_id
        ),

    CONSTRAINT approval_policy_version_chain_members_policy_chain_fkey
        FOREIGN KEY (
            organization_id,
            policy_id,
            version,
            cluster,
            program_id,
            policy_version_pda
        )
        REFERENCES approval_policy_version_chain_accounts (
            organization_id,
            policy_id,
            version,
            cluster,
            program_id,
            policy_version_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_version_chain_members_member_chain_fkey
        FOREIGN KEY (
            organization_id,
            member_id,
            cluster,
            program_id,
            member_pda
        )
        REFERENCES member_chain_accounts (
            organization_id,
            member_id,
            cluster,
            program_id,
            member_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_version_chain_members_order_range
        CHECK (
            member_order BETWEEN 0 AND 7
        ),

    CONSTRAINT approval_policy_version_chain_members_order_unique
        UNIQUE (
            organization_id,
            policy_id,
            version,
            cluster,
            program_id,
            member_order
        ),

    CONSTRAINT approval_policy_version_chain_members_slot_nonnegative
        CHECK (
            observed_slot >= 0
        )
);


CREATE INDEX recipient_chain_accounts_active_index
    ON recipient_chain_accounts (
        organization_id,
        active
    );

CREATE INDEX vault_chain_accounts_active_index
    ON vault_chain_accounts (
        organization_id,
        active
    );

CREATE INDEX approval_policy_version_chain_accounts_enabled_index
    ON approval_policy_version_chain_accounts (
        organization_id,
        enabled
    );


COMMENT ON TABLE member_chain_accounts IS
    'Authoritative indexed snapshots of on-chain Member PDA accounts.';

COMMENT ON TABLE member_wallet_chain_accounts IS
    'Indexed MemberWallet PDA mappings that enforce one wallet mapping per organization.';

COMMENT ON TABLE recipient_chain_accounts IS
    'Authoritative indexed snapshots of on-chain Recipient PDA accounts.';

COMMENT ON TABLE vault_chain_accounts IS
    'Indexed VaultState PDA snapshots plus the derived vault token-account balance used by the application read model.';

COMMENT ON TABLE approval_policy_version_chain_accounts IS
    'Authoritative indexed snapshots of immutable ApprovalPolicyVersion PDA accounts.';

COMMENT ON TABLE approval_policy_version_chain_members IS
    'Normalized ordered projection of the eligible_members vector stored in each ApprovalPolicyVersion PDA.';
