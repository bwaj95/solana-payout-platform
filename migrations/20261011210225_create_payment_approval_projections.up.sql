CREATE TABLE payment_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    payment_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    payment_pda TEXT NOT NULL,

    created_by_member_id NUMERIC(20, 0) NOT NULL,
    created_by_member_pda TEXT NOT NULL,

    recipient_id NUMERIC(20, 0) NOT NULL,
    recipient_pda TEXT NOT NULL,
    destination_wallet TEXT NOT NULL,
    recipient_wallet_revision BIGINT NOT NULL,

    vault_id NUMERIC(20, 0) NOT NULL,
    vault_pda TEXT NOT NULL,

    amount NUMERIC(20, 0) NOT NULL,

    policy_id NUMERIC(20, 0) NOT NULL,
    policy_version NUMERIC(20, 0) NOT NULL,
    policy_version_pda TEXT NOT NULL,

    payment_revision BIGINT NOT NULL,
    settlement_rail TEXT NOT NULL,
    execute_after_unix BIGINT NOT NULL,

    terms_hash BYTEA NOT NULL,

    payment_state TEXT NOT NULL,
    reservation_state TEXT NOT NULL,

    bump SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT payment_chain_accounts_pkey
        PRIMARY KEY (
            organization_id,
            payment_id,
            cluster,
            program_id
        ),

    CONSTRAINT payment_chain_accounts_application_payment_fkey
        FOREIGN KEY (
            organization_id,
            payment_id
        )
        REFERENCES payments (
            organization_id,
            payment_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT payment_chain_accounts_organization_chain_fkey
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

    CONSTRAINT payment_chain_accounts_creator_chain_fkey
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

    CONSTRAINT payment_chain_accounts_recipient_chain_fkey
        FOREIGN KEY (
            organization_id,
            recipient_id,
            cluster,
            program_id,
            recipient_pda
        )
        REFERENCES recipient_chain_accounts (
            organization_id,
            recipient_id,
            cluster,
            program_id,
            recipient_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT payment_chain_accounts_vault_chain_fkey
        FOREIGN KEY (
            organization_id,
            vault_id,
            cluster,
            program_id,
            vault_pda
        )
        REFERENCES vault_chain_accounts (
            organization_id,
            vault_id,
            cluster,
            program_id,
            vault_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT payment_chain_accounts_policy_chain_fkey
        FOREIGN KEY (
            organization_id,
            policy_id,
            policy_version,
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

    CONSTRAINT payment_chain_accounts_address_unique
        UNIQUE (
            cluster,
            program_id,
            payment_pda
        ),

    CONSTRAINT payment_chain_accounts_identity_with_pda_unique
        UNIQUE (
            organization_id,
            payment_id,
            cluster,
            program_id,
            payment_pda
        ),

    CONSTRAINT payment_chain_accounts_recipient_revision_u32_range
        CHECK (
            recipient_wallet_revision >= 0
            AND recipient_wallet_revision <= 4294967295
        ),

    CONSTRAINT payment_chain_accounts_payment_revision_u32_range
        CHECK (
            payment_revision >= 0
            AND payment_revision <= 4294967295
        ),

    CONSTRAINT payment_chain_accounts_amount_u64_range
        CHECK (
            amount > 0
            AND amount <= 18446744073709551615
        ),

    CONSTRAINT payment_chain_accounts_settlement_rail_valid
        CHECK (
            settlement_rail IN (
                'public_sol',
                'public_spl',
                'private_magic_block'
            )
        ),

    CONSTRAINT payment_chain_accounts_payment_state_valid
        CHECK (
            payment_state IN (
                'pending_approval',
                'awaiting_funds',
                'approved',
                'paid',
                'cancelled'
            )
        ),

    CONSTRAINT payment_chain_accounts_reservation_state_valid
        CHECK (
            reservation_state IN (
                'none',
                'held',
                'consumed',
                'released'
            )
        ),

    CONSTRAINT payment_chain_accounts_terms_hash_length
        CHECK (
            octet_length(terms_hash) = 32
        ),

    CONSTRAINT payment_chain_accounts_destination_length
        CHECK (
            char_length(destination_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT payment_chain_accounts_pda_length
        CHECK (
            char_length(payment_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT payment_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT payment_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT payment_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);

CREATE INDEX payment_chain_accounts_state_index
    ON payment_chain_accounts (
        organization_id,
        payment_state,
        reservation_state
    );

CREATE INDEX payment_chain_accounts_execution_index
    ON payment_chain_accounts (
        organization_id,
        execute_after_unix
    )
    WHERE payment_state = 'approved';


-- Approval has no application-allocated numeric ID. Its PDA is the stable
-- identity and its remaining fields are immutable authorization snapshots.
CREATE TABLE approval_chain_accounts (
    organization_id NUMERIC(20, 0) NOT NULL,
    payment_id NUMERIC(20, 0) NOT NULL,

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    organization_pda TEXT NOT NULL,
    payment_pda TEXT NOT NULL,
    approval_pda TEXT NOT NULL,

    payment_revision BIGINT NOT NULL,

    policy_id NUMERIC(20, 0) NOT NULL,
    policy_version NUMERIC(20, 0) NOT NULL,
    policy_version_pda TEXT NOT NULL,

    member_id NUMERIC(20, 0) NOT NULL,
    member_pda TEXT NOT NULL,
    member_authorization_revision NUMERIC(20, 0) NOT NULL,
    authorization_wallet TEXT NOT NULL,

    approved_at_unix BIGINT NOT NULL,
    terms_hash BYTEA NOT NULL,

    bump SMALLINT NOT NULL,

    observed_slot BIGINT NOT NULL,
    observed_commitment TEXT NOT NULL,

    first_observed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_synced_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT approval_chain_accounts_pkey
        PRIMARY KEY (
            cluster,
            program_id,
            approval_pda
        ),

    CONSTRAINT approval_chain_accounts_organization_chain_fkey
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

    CONSTRAINT approval_chain_accounts_payment_chain_fkey
        FOREIGN KEY (
            organization_id,
            payment_id,
            cluster,
            program_id,
            payment_pda
        )
        REFERENCES payment_chain_accounts (
            organization_id,
            payment_id,
            cluster,
            program_id,
            payment_pda
        )
        ON DELETE RESTRICT,

    CONSTRAINT approval_chain_accounts_policy_chain_fkey
        FOREIGN KEY (
            organization_id,
            policy_id,
            policy_version,
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

    CONSTRAINT approval_chain_accounts_member_chain_fkey
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

    -- Prevents the same authorization snapshot from being indexed twice,
    -- even if a bad indexer attempted to associate two addresses with it.
    CONSTRAINT approval_chain_accounts_snapshot_unique
        UNIQUE (
            organization_id,
            payment_id,
            payment_revision,
            member_id,
            member_authorization_revision,
            cluster,
            program_id
        ),

    CONSTRAINT approval_chain_accounts_payment_revision_u32_range
        CHECK (
            payment_revision >= 0
            AND payment_revision <= 4294967295
        ),

    CONSTRAINT approval_chain_accounts_member_revision_u64_range
        CHECK (
            member_authorization_revision >= 0
            AND member_authorization_revision <= 18446744073709551615
        ),

    CONSTRAINT approval_chain_accounts_wallet_length
        CHECK (
            char_length(authorization_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT approval_chain_accounts_pda_length
        CHECK (
            char_length(approval_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT approval_chain_accounts_terms_hash_length
        CHECK (
            octet_length(terms_hash) = 32
        ),

    CONSTRAINT approval_chain_accounts_bump_range
        CHECK (
            bump BETWEEN 0 AND 255
        ),

    CONSTRAINT approval_chain_accounts_slot_nonnegative
        CHECK (
            observed_slot >= 0
        ),

    CONSTRAINT approval_chain_accounts_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        )
);

CREATE INDEX approval_chain_accounts_payment_index
    ON approval_chain_accounts (
        organization_id,
        payment_id,
        payment_revision,
        approved_at_unix
    );


-- Append-only audit log of Anchor events. The JSON payload allows new event
-- versions to be indexed without requiring an immediate schema migration.
CREATE TABLE indexed_program_events (
    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    transaction_signature TEXT NOT NULL,
    event_index INTEGER NOT NULL,

    slot BIGINT NOT NULL,
    block_time TIMESTAMPTZ,
    observed_commitment TEXT NOT NULL,

    event_name TEXT NOT NULL,

    organization_pda TEXT,
    payment_pda TEXT,

    payload JSONB NOT NULL,

    indexed_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT indexed_program_events_pkey
        PRIMARY KEY (
            cluster,
            program_id,
            transaction_signature,
            event_index
        ),

    CONSTRAINT indexed_program_events_cluster_valid
        CHECK (
            cluster IN (
                'localnet',
                'devnet',
                'testnet',
                'mainnet-beta'
            )
        ),

    CONSTRAINT indexed_program_events_signature_length
        CHECK (
            char_length(transaction_signature) BETWEEN 64 AND 128
        ),

    CONSTRAINT indexed_program_events_event_index_nonnegative
        CHECK (
            event_index >= 0
        ),

    CONSTRAINT indexed_program_events_slot_nonnegative
        CHECK (
            slot >= 0
        ),

    CONSTRAINT indexed_program_events_commitment_valid
        CHECK (
            observed_commitment IN ('confirmed', 'finalized')
        ),

    CONSTRAINT indexed_program_events_name_length
        CHECK (
            char_length(event_name) BETWEEN 1 AND 120
        ),

    CONSTRAINT indexed_program_events_organization_pda_length
        CHECK (
            organization_pda IS NULL
            OR char_length(organization_pda) BETWEEN 32 AND 44
        ),

    CONSTRAINT indexed_program_events_payment_pda_length
        CHECK (
            payment_pda IS NULL
            OR char_length(payment_pda) BETWEEN 32 AND 44
        )
);

CREATE INDEX indexed_program_events_organization_index
    ON indexed_program_events (
        cluster,
        program_id,
        organization_pda,
        slot DESC,
        event_index DESC
    );

CREATE INDEX indexed_program_events_payment_index
    ON indexed_program_events (
        cluster,
        program_id,
        payment_pda,
        slot DESC,
        event_index DESC
    )
    WHERE payment_pda IS NOT NULL;


-- Each indexer process owns a named cursor. This prevents a restart from
-- forcing a full historical scan every time.
CREATE TABLE indexer_cursors (
    indexer_name TEXT NOT NULL,
    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    last_processed_signature TEXT,
    last_processed_slot BIGINT NOT NULL DEFAULT 0,

    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT indexer_cursors_pkey
        PRIMARY KEY (
            indexer_name,
            cluster,
            program_id
        ),

    CONSTRAINT indexer_cursors_name_length
        CHECK (
            char_length(indexer_name) BETWEEN 1 AND 120
        ),

    CONSTRAINT indexer_cursors_cluster_valid
        CHECK (
            cluster IN (
                'localnet',
                'devnet',
                'testnet',
                'mainnet-beta'
            )
        ),

    CONSTRAINT indexer_cursors_signature_length
        CHECK (
            last_processed_signature IS NULL
            OR char_length(last_processed_signature) BETWEEN 64 AND 128
        ),

    CONSTRAINT indexer_cursors_slot_nonnegative
        CHECK (
            last_processed_slot >= 0
        )
);

CREATE TRIGGER indexer_cursors_set_updated_at
BEFORE UPDATE ON indexer_cursors
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();


COMMENT ON TABLE payment_chain_accounts IS
    'Authoritative indexed snapshots of on-chain Payment PDA accounts.';

COMMENT ON TABLE approval_chain_accounts IS
    'Indexed immutable Approval PDA snapshots; no application-generated approval ID is required.';

COMMENT ON TABLE indexed_program_events IS
    'Append-only idempotent audit log of decoded Anchor events.';

COMMENT ON TABLE indexer_cursors IS
    'Durable progress cursors used by resumable Solana indexer processes.';
