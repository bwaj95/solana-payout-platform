-- All application-generated identifiers use BIGINT sequences, but the
-- columns are NUMERIC(20, 0) so they can also represent any imported u64.
--
-- The generated value is passed directly to the Anchor instruction and
-- therefore becomes the exact member_id, recipient_id, or vault_id used
-- in the corresponding PDA seeds.

CREATE SEQUENCE organization_members_member_id_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    NO CYCLE;

CREATE SEQUENCE recipients_recipient_id_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    NO CYCLE;

CREATE SEQUENCE vaults_vault_id_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    NO CYCLE;


CREATE TABLE organization_members (
    organization_id NUMERIC(20, 0) NOT NULL,

    member_id NUMERIC(20, 0)
        NOT NULL
        DEFAULT nextval('organization_members_member_id_seq')::NUMERIC,

    display_name TEXT NOT NULL,

    -- Optional employee ID, accounting ID, or another identifier from
    -- the organization's existing systems.
    external_reference TEXT,

    -- These fields record what the application intends to create
    -- on-chain. The indexed Member account will be authoritative after
    -- the transaction is confirmed.
    requested_authorized_wallet TEXT NOT NULL,
    requested_roles INTEGER NOT NULL,

    lifecycle_status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT organization_members_pkey
        PRIMARY KEY (
            organization_id,
            member_id
        ),

    CONSTRAINT organization_members_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT organization_members_id_u64_range
        CHECK (
            member_id >= 0
            AND member_id <= 18446744073709551615
        ),

    CONSTRAINT organization_members_name_length
        CHECK (
            char_length(btrim(display_name)) BETWEEN 1 AND 120
        ),

    CONSTRAINT organization_members_external_reference_length
        CHECK (
            external_reference IS NULL
            OR char_length(btrim(external_reference)) BETWEEN 1 AND 120
        ),

    CONSTRAINT organization_members_wallet_length
        CHECK (
            char_length(requested_authorized_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT organization_members_roles_u16_range
        CHECK (
            requested_roles >= 0
            AND requested_roles <= 65535
        ),

    CONSTRAINT organization_members_roles_nonzero
        CHECK (requested_roles <> 0),

    CONSTRAINT organization_members_lifecycle_status_valid
        CHECK (
            lifecycle_status IN (
                'draft',
                'provisioning',
                'active',
                'archived'
            )
        )
);

ALTER SEQUENCE organization_members_member_id_seq
    OWNED BY organization_members.member_id;

CREATE UNIQUE INDEX organization_members_external_reference_unique
    ON organization_members (
        organization_id,
        external_reference
    )
    WHERE external_reference IS NOT NULL;


CREATE TABLE recipients (
    organization_id NUMERIC(20, 0) NOT NULL,

    recipient_id NUMERIC(20, 0)
        NOT NULL
        DEFAULT nextval('recipients_recipient_id_seq')::NUMERIC,

    display_name TEXT NOT NULL,

    -- Can hold a vendor code, employee code, customer code, etc.
    -- A future invoices table can reference:
    -- (organization_id, recipient_id).
    external_reference TEXT,

    requested_destination_wallet TEXT NOT NULL,

    lifecycle_status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT recipients_pkey
        PRIMARY KEY (
            organization_id,
            recipient_id
        ),

    CONSTRAINT recipients_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT recipients_id_u64_range
        CHECK (
            recipient_id >= 0
            AND recipient_id <= 18446744073709551615
        ),

    CONSTRAINT recipients_name_length
        CHECK (
            char_length(btrim(display_name)) BETWEEN 1 AND 160
        ),

    CONSTRAINT recipients_external_reference_length
        CHECK (
            external_reference IS NULL
            OR char_length(btrim(external_reference)) BETWEEN 1 AND 120
        ),

    CONSTRAINT recipients_destination_wallet_length
        CHECK (
            char_length(requested_destination_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT recipients_lifecycle_status_valid
        CHECK (
            lifecycle_status IN (
                'draft',
                'provisioning',
                'active',
                'archived'
            )
        )
);

ALTER SEQUENCE recipients_recipient_id_seq
    OWNED BY recipients.recipient_id;

CREATE UNIQUE INDEX recipients_external_reference_unique
    ON recipients (
        organization_id,
        external_reference
    )
    WHERE external_reference IS NOT NULL;


CREATE TABLE vaults (
    organization_id NUMERIC(20, 0) NOT NULL,

    vault_id NUMERIC(20, 0)
        NOT NULL
        DEFAULT nextval('vaults_vault_id_seq')::NUMERIC,

    -- Examples: "Payroll USDC", "Operations USDC", "Vendor Payments".
    name TEXT NOT NULL,

    -- Explains why this vault exists without overloading the mint address.
    purpose TEXT NOT NULL,

    asset_kind TEXT NOT NULL,
    mint_address TEXT,

    lifecycle_status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT vaults_pkey
        PRIMARY KEY (
            organization_id,
            vault_id
        ),

    CONSTRAINT vaults_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT vaults_id_u64_range
        CHECK (
            vault_id >= 0
            AND vault_id <= 18446744073709551615
        ),

    CONSTRAINT vaults_name_length
        CHECK (
            char_length(btrim(name)) BETWEEN 1 AND 120
        ),

    CONSTRAINT vaults_purpose_length
        CHECK (
            char_length(btrim(purpose)) BETWEEN 1 AND 500
        ),

    CONSTRAINT vaults_asset_kind_valid
        CHECK (
            asset_kind IN (
                'native_sol',
                'spl'
            )
        ),

    -- Native SOL does not have a mint. An SPL vault must have one.
    CONSTRAINT vaults_asset_configuration_valid
        CHECK (
            (
                asset_kind = 'native_sol'
                AND mint_address IS NULL
            )
            OR
            (
                asset_kind = 'spl'
                AND mint_address IS NOT NULL
                AND char_length(mint_address) BETWEEN 32 AND 44
            )
        ),

    CONSTRAINT vaults_lifecycle_status_valid
        CHECK (
            lifecycle_status IN (
                'draft',
                'provisioning',
                'active',
                'archived'
            )
        )
);

ALTER SEQUENCE vaults_vault_id_seq
    OWNED BY vaults.vault_id;

CREATE UNIQUE INDEX vaults_name_unique
    ON vaults (
        organization_id,
        lower(name)
    );


-- Keep updated_at trustworthy even if a future application update forgets
-- to set it explicitly.
CREATE FUNCTION set_row_updated_at()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$;

CREATE TRIGGER organizations_set_updated_at
BEFORE UPDATE ON organizations
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();

CREATE TRIGGER organization_members_set_updated_at
BEFORE UPDATE ON organization_members
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();

CREATE TRIGGER recipients_set_updated_at
BEFORE UPDATE ON recipients
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();

CREATE TRIGGER vaults_set_updated_at
BEFORE UPDATE ON vaults
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();


COMMENT ON TABLE organization_members IS
    'Application member records. member_id is also the exact u64 used in the Member PDA seeds.';

COMMENT ON TABLE recipients IS
    'Application recipient records. recipient_id is also the exact u64 used in the Recipient PDA seeds.';

COMMENT ON TABLE vaults IS
    'Application vault records. vault_id is also the exact u64 used in the VaultState PDA seeds.';
