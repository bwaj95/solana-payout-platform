CREATE SEQUENCE approval_policies_policy_id_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    NO CYCLE;

CREATE SEQUENCE payments_payment_id_seq
    AS BIGINT
    START WITH 1
    INCREMENT BY 1
    NO CYCLE;


-- A policy is an application-level aggregate such as "Payroll approvals".
-- There is no ApprovalPolicy parent PDA. Each immutable version gets its
-- own ApprovalPolicyVersion PDA.
CREATE TABLE approval_policies (
    organization_id NUMERIC(20, 0) NOT NULL,

    policy_id NUMERIC(20, 0)
        NOT NULL
        DEFAULT nextval('approval_policies_policy_id_seq')::NUMERIC,

    name TEXT NOT NULL,
    description TEXT,

    -- The next version to allocate for this policy. The backend will
    -- increment this inside a transaction and use the returned value
    -- in the PolicyVersion PDA seeds.
    next_version NUMERIC(20, 0) NOT NULL DEFAULT 1,

    lifecycle_status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT approval_policies_pkey
        PRIMARY KEY (
            organization_id,
            policy_id
        ),

    CONSTRAINT approval_policies_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT approval_policies_id_u64_range
        CHECK (
            policy_id >= 0
            AND policy_id <= 18446744073709551615
        ),

    -- u64::MAX + 1 is allowed as an exhausted sentinel after allocating
    -- the final representable u64 version.
    CONSTRAINT approval_policies_next_version_range
        CHECK (
            next_version >= 1
            AND next_version <= 18446744073709551616
        ),

    CONSTRAINT approval_policies_name_length
        CHECK (
            char_length(btrim(name)) BETWEEN 1 AND 120
        ),

    CONSTRAINT approval_policies_description_length
        CHECK (
            description IS NULL
            OR char_length(btrim(description)) BETWEEN 1 AND 1000
        ),

    CONSTRAINT approval_policies_lifecycle_status_valid
        CHECK (
            lifecycle_status IN (
                'draft',
                'active',
                'archived'
            )
        )
);

ALTER SEQUENCE approval_policies_policy_id_seq
    OWNED BY approval_policies.policy_id;

CREATE UNIQUE INDEX approval_policies_name_unique
    ON approval_policies (
        organization_id,
        lower(name)
    );


CREATE TABLE approval_policy_versions (
    organization_id NUMERIC(20, 0) NOT NULL,
    policy_id NUMERIC(20, 0) NOT NULL,
    version NUMERIC(20, 0) NOT NULL,

    threshold SMALLINT NOT NULL,

    -- Application member identity. The corresponding on-chain account
    -- stores the Member PDA rather than this numeric member ID.
    created_by_member_id NUMERIC(20, 0) NOT NULL,

    change_note TEXT,

    lifecycle_status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT approval_policy_versions_pkey
        PRIMARY KEY (
            organization_id,
            policy_id,
            version
        ),

    CONSTRAINT approval_policy_versions_policy_fkey
        FOREIGN KEY (
            organization_id,
            policy_id
        )
        REFERENCES approval_policies (
            organization_id,
            policy_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_versions_creator_fkey
        FOREIGN KEY (
            organization_id,
            created_by_member_id
        )
        REFERENCES organization_members (
            organization_id,
            member_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_versions_version_u64_range
        CHECK (
            version >= 1
            AND version <= 18446744073709551615
        ),

    CONSTRAINT approval_policy_versions_threshold_range
        CHECK (
            threshold BETWEEN 1 AND 8
        ),

    CONSTRAINT approval_policy_versions_change_note_length
        CHECK (
            change_note IS NULL
            OR char_length(btrim(change_note)) BETWEEN 1 AND 1000
        ),

    CONSTRAINT approval_policy_versions_lifecycle_status_valid
        CHECK (
            lifecycle_status IN (
                'draft',
                'provisioning',
                'active',
                'archived'
            )
        )
);


CREATE TABLE approval_policy_version_members (
    organization_id NUMERIC(20, 0) NOT NULL,
    policy_id NUMERIC(20, 0) NOT NULL,
    version NUMERIC(20, 0) NOT NULL,
    member_id NUMERIC(20, 0) NOT NULL,

    -- Preserves the deterministic ordering sent to the Anchor instruction.
    member_order SMALLINT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT approval_policy_version_members_pkey
        PRIMARY KEY (
            organization_id,
            policy_id,
            version,
            member_id
        ),

    CONSTRAINT approval_policy_version_members_version_fkey
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

    CONSTRAINT approval_policy_version_members_member_fkey
        FOREIGN KEY (
            organization_id,
            member_id
        )
        REFERENCES organization_members (
            organization_id,
            member_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_policy_version_members_order_range
        CHECK (
            member_order BETWEEN 0 AND 7
        ),

    CONSTRAINT approval_policy_version_members_order_unique
        UNIQUE (
            organization_id,
            policy_id,
            version,
            member_order
        )
);


CREATE TABLE payments (
    organization_id NUMERIC(20, 0) NOT NULL,

    payment_id NUMERIC(20, 0)
        NOT NULL
        DEFAULT nextval('payments_payment_id_seq')::NUMERIC,

    description TEXT NOT NULL,
    external_reference TEXT,

    created_by_member_id NUMERIC(20, 0) NOT NULL,
    recipient_id NUMERIC(20, 0) NOT NULL,
    vault_id NUMERIC(20, 0) NOT NULL,

    policy_id NUMERIC(20, 0) NOT NULL,
    policy_version NUMERIC(20, 0) NOT NULL,

    amount NUMERIC(20, 0) NOT NULL,
    settlement_rail TEXT NOT NULL,
    execute_after TIMESTAMPTZ NOT NULL,

    -- This is the lifecycle of the application record. PaymentState and
    -- ReservationState will live in payment_chain_accounts.
    lifecycle_status TEXT NOT NULL DEFAULT 'draft',

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT payments_pkey
        PRIMARY KEY (
            organization_id,
            payment_id
        ),

    CONSTRAINT payments_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT payments_creator_fkey
        FOREIGN KEY (
            organization_id,
            created_by_member_id
        )
        REFERENCES organization_members (
            organization_id,
            member_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT payments_recipient_fkey
        FOREIGN KEY (
            organization_id,
            recipient_id
        )
        REFERENCES recipients (
            organization_id,
            recipient_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT payments_vault_fkey
        FOREIGN KEY (
            organization_id,
            vault_id
        )
        REFERENCES vaults (
            organization_id,
            vault_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT payments_policy_version_fkey
        FOREIGN KEY (
            organization_id,
            policy_id,
            policy_version
        )
        REFERENCES approval_policy_versions (
            organization_id,
            policy_id,
            version
        )
        ON DELETE RESTRICT,

    CONSTRAINT payments_id_u64_range
        CHECK (
            payment_id >= 0
            AND payment_id <= 18446744073709551615
        ),

    CONSTRAINT payments_amount_u64_range
        CHECK (
            amount > 0
            AND amount <= 18446744073709551615
        ),

    CONSTRAINT payments_description_length
        CHECK (
            char_length(btrim(description)) BETWEEN 1 AND 500
        ),

    CONSTRAINT payments_external_reference_length
        CHECK (
            external_reference IS NULL
            OR char_length(btrim(external_reference)) BETWEEN 1 AND 160
        ),

    CONSTRAINT payments_settlement_rail_valid
        CHECK (
            settlement_rail IN (
                'public_sol',
                'public_spl',
                'private_magic_block'
            )
        ),

    CONSTRAINT payments_lifecycle_status_valid
        CHECK (
            lifecycle_status IN (
                'draft',
                'provisioning',
                'active',
                'archived'
            )
        )
);

ALTER SEQUENCE payments_payment_id_seq
    OWNED BY payments.payment_id;

CREATE UNIQUE INDEX payments_external_reference_unique
    ON payments (
        organization_id,
        external_reference
    )
    WHERE external_reference IS NOT NULL;

CREATE INDEX payments_recipient_index
    ON payments (
        organization_id,
        recipient_id,
        created_at DESC
    );

CREATE INDEX payments_vault_index
    ON payments (
        organization_id,
        vault_id,
        created_at DESC
    );

CREATE INDEX payments_policy_version_index
    ON payments (
        organization_id,
        policy_id,
        policy_version
    );


CREATE TRIGGER approval_policies_set_updated_at
BEFORE UPDATE ON approval_policies
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();

CREATE TRIGGER approval_policy_versions_set_updated_at
BEFORE UPDATE ON approval_policy_versions
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();

CREATE TRIGGER payments_set_updated_at
BEFORE UPDATE ON payments
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();


COMMENT ON TABLE approval_policies IS
    'Application policy aggregates. A policy groups its immutable on-chain policy versions.';

COMMENT ON TABLE approval_policy_versions IS
    'Application representation of a specific policy version. Its composite key supplies the PolicyVersion PDA seed values.';

COMMENT ON TABLE approval_policy_version_members IS
    'Ordered eligible-member configuration for a specific approval policy version.';

COMMENT ON TABLE payments IS
    'Application payment records. payment_id is also the exact u64 used in the Payment PDA seeds.';
