CREATE SEQUENCE organizations_organization_id_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    NO CYCLE;

CREATE TABLE organizations (
    organization_id NUMERIC(20, 0)
        NOT NULL
        DEFAULT nextval('organizations_organization_id_seq')::NUMERIC,

    name TEXT NOT NULL,

    -- Application lifecycle. This is different from the on-chain
    -- Organization.paused emergency-control flag.
    status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT organizations_pkey
        PRIMARY KEY (organization_id),

    CONSTRAINT organizations_id_u64_range
        CHECK (
            organization_id >= 0
            AND organization_id <= 18446744073709551615
        ),

    CONSTRAINT organizations_name_length
        CHECK (
            char_length(btrim(name)) >= 1
            AND char_length(btrim(name)) <= 120
        ),

    CONSTRAINT organizations_status_valid
        CHECK (
            status IN (
                'draft',
                'provisioning',
                'active',
                'archived'
            )
        )
);

ALTER SEQUENCE organizations_organization_id_seq
    OWNED BY organizations.organization_id;

COMMENT ON TABLE organizations IS
    'Application-owned organization aggregate. organization_id is also the u64 used in the Organization PDA seeds.';

COMMENT ON COLUMN organizations.organization_id IS
    'Stable organization identity allocated before the on-chain initialization transaction and passed directly into the PDA seeds.';

COMMENT ON COLUMN organizations.status IS
    'Application lifecycle status; separate from the on-chain paused flag.';


CREATE TABLE organization_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,
    organization_pda TEXT NOT NULL,

    -- These are authoritative fields read from the Organization PDA.
    creator_wallet TEXT NOT NULL,
    owner_member_pda TEXT NOT NULL,
    paused BOOLEAN NOT NULL,
    bump SMALLINT NOT NULL,

    -- Records how fresh this projection is.
    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT organization_chain_accounts_pkey
        PRIMARY KEY (
            organization_id,
            cluster,
            program_id
        ),

    CONSTRAINT organization_chain_accounts_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT organization_chain_accounts_address_unique
        UNIQUE (
            cluster,
            program_id,
            organization_pda
        ),

    -- Organization PDA seeds:
    -- [
    --   ORGANIZATION_SEED,
    --   creator_wallet,
    --   organization_id
    -- ]
    CONSTRAINT organization_chain_accounts_seed_identity_unique
        UNIQUE (
            cluster,
            program_id,
            creator_wallet,
            organization_id
        ),

    CONSTRAINT organization_chain_accounts_owner_member_unique
        UNIQUE (
            cluster,
            program_id,
            owner_member_pda
        ),

    CONSTRAINT organization_chain_accounts_cluster_valid
        CHECK (
            cluster IN (
                'localnet',
                'devnet',
                'testnet',
                'mainnet-beta'
            )
        ),

    CONSTRAINT organization_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN (
                'confirmed',
                'finalized'
            )
        ),

    CONSTRAINT organization_chain_accounts_program_id_length
        CHECK (
            char_length(program_id) BETWEEN 32 AND 44
        ),

    CONSTRAINT organization_chain_accounts_pda_length
        CHECK (
            char_length(organization_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT organization_chain_accounts_creator_length
        CHECK (
            char_length(creator_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT organization_chain_accounts_owner_member_length
        CHECK (
            char_length(owner_member_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT organization_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT organization_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        )
);

CREATE INDEX organization_chain_accounts_sync_index
    ON organization_chain_accounts (
        cluster,
        program_id,
        last_synced_at
    );

COMMENT ON TABLE organization_chain_accounts IS
    'Indexed on-chain Organization PDA state. The application organization row remains separate from this chain projection.';

COMMENT ON COLUMN organization_chain_accounts.organization_id IS
    'References organizations.organization_id, which is also the exact u64 stored in and used to derive the Organization PDA.';

COMMENT ON COLUMN organization_chain_accounts.paused IS
    'Authoritative emergency pause state read from the Organization PDA.';
