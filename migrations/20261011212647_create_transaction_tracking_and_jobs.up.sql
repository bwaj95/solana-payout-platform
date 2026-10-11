CREATE TABLE transaction_submissions (
    submission_id BIGINT GENERATED ALWAYS AS IDENTITY,

    -- The client or backend can reuse this key when an HTTP request is
    -- retried. The unique constraint prevents creating a second submission.
    idempotency_key UUID NOT NULL DEFAULT gen_random_uuid(),

    organization_id NUMERIC(20, 0) NOT NULL,
    payment_id NUMERIC(20, 0),

    cluster TEXT NOT NULL,
    program_id TEXT NOT NULL,

    instruction_name TEXT NOT NULL,
    signer_wallet TEXT NOT NULL,

    -- Flexible identifiers for instructions that do not operate on a payment.
    -- Example: {"vault_id": "3"} or {"policy_id": "2", "version": "1"}.
    entity_key JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Sanitized instruction input. Never store a private key or secret here.
    request_payload JSONB NOT NULL DEFAULT '{}'::JSONB,

    transaction_signature TEXT,
    last_valid_block_height BIGINT,

    status TEXT NOT NULL DEFAULT 'prepared',

    confirmed_slot BIGINT,
    observed_commitment TEXT,

    submitted_at TIMESTAMPTZ,
    last_checked_at TIMESTAMPTZ,
    confirmed_at TIMESTAMPTZ,
    finalized_at TIMESTAMPTZ,
    failed_at TIMESTAMPTZ,

    error_code TEXT,
    error_message TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT transaction_submissions_pkey
        PRIMARY KEY (submission_id),

    CONSTRAINT transaction_submissions_idempotency_unique
        UNIQUE (idempotency_key),

    CONSTRAINT transaction_submissions_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT transaction_submissions_payment_fkey
        FOREIGN KEY (
            organization_id,
            payment_id
        )
        REFERENCES payments (
            organization_id,
            payment_id
        )
        ON DELETE RESTRICT,

    CONSTRAINT transaction_submissions_cluster_valid
        CHECK (
            cluster IN (
                'localnet',
                'devnet',
                'testnet',
                'mainnet-beta'
            )
        ),

    CONSTRAINT transaction_submissions_status_valid
        CHECK (
            status IN (
                'prepared',
                'submitted',
                'unknown',
                'confirmed',
                'finalized',
                'failed',
                'expired'
            )
        ),

    -- Once submission has happened, its precomputed signature must be stored.
    -- Failed preparation is allowed to have no signature.
    CONSTRAINT transaction_submissions_signature_required
        CHECK (
            status NOT IN (
                'submitted',
                'unknown',
                'confirmed',
                'finalized',
                'expired'
            )
            OR transaction_signature IS NOT NULL
        ),

    CONSTRAINT transaction_submissions_signature_length
        CHECK (
            transaction_signature IS NULL
            OR char_length(transaction_signature) BETWEEN 64 AND 128
        ),

    CONSTRAINT transaction_submissions_program_id_length
        CHECK (
            char_length(program_id) BETWEEN 32 AND 44
        ),

    CONSTRAINT transaction_submissions_signer_length
        CHECK (
            char_length(signer_wallet) BETWEEN 32 AND 44
        ),

    CONSTRAINT transaction_submissions_instruction_name_length
        CHECK (
            char_length(instruction_name) BETWEEN 1 AND 120
        ),

    CONSTRAINT transaction_submissions_entity_key_object
        CHECK (
            jsonb_typeof(entity_key) = 'object'
        ),

    CONSTRAINT transaction_submissions_request_payload_object
        CHECK (
            jsonb_typeof(request_payload) = 'object'
        ),

    CONSTRAINT transaction_submissions_block_height_nonnegative
        CHECK (
            last_valid_block_height IS NULL
            OR last_valid_block_height >= 0
        ),

    CONSTRAINT transaction_submissions_confirmed_slot_nonnegative
        CHECK (
            confirmed_slot IS NULL
            OR confirmed_slot >= 0
        ),

    CONSTRAINT transaction_submissions_commitment_valid
        CHECK (
            observed_commitment IS NULL
            OR observed_commitment IN ('confirmed', 'finalized')
        )
);

CREATE UNIQUE INDEX transaction_submissions_signature_unique
    ON transaction_submissions (
        cluster,
        program_id,
        transaction_signature
    )
    WHERE transaction_signature IS NOT NULL;

CREATE INDEX transaction_submissions_reconciliation_index
    ON transaction_submissions (
        status,
        last_checked_at,
        created_at
    )
    WHERE status IN (
        'submitted',
        'unknown',
        'confirmed'
    );

CREATE INDEX transaction_submissions_payment_index
    ON transaction_submissions (
        organization_id,
        payment_id,
        created_at DESC
    )
    WHERE payment_id IS NOT NULL;


-- Append-only history makes transaction-state changes auditable rather than
-- leaving only the most recent status in transaction_submissions.
CREATE TABLE transaction_submission_status_history (
    history_id BIGINT GENERATED ALWAYS AS IDENTITY,
    submission_id BIGINT NOT NULL,

    previous_status TEXT,
    new_status TEXT NOT NULL,

    error_code TEXT,
    error_message TEXT,

    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT transaction_submission_status_history_pkey
        PRIMARY KEY (history_id),

    CONSTRAINT transaction_submission_status_history_submission_fkey
        FOREIGN KEY (submission_id)
        REFERENCES transaction_submissions (submission_id)
        ON DELETE RESTRICT,

    CONSTRAINT transaction_submission_status_history_previous_status_valid
        CHECK (
            previous_status IS NULL
            OR previous_status IN (
                'prepared',
                'submitted',
                'unknown',
                'confirmed',
                'finalized',
                'failed',
                'expired'
            )
        ),

    CONSTRAINT transaction_submission_status_history_new_status_valid
        CHECK (
            new_status IN (
                'prepared',
                'submitted',
                'unknown',
                'confirmed',
                'finalized',
                'failed',
                'expired'
            )
        )
);

CREATE INDEX transaction_submission_status_history_submission_index
    ON transaction_submission_status_history (
        submission_id,
        recorded_at
    );


CREATE FUNCTION record_transaction_submission_status_change()
RETURNS TRIGGER
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        INSERT INTO transaction_submission_status_history (
            submission_id,
            previous_status,
            new_status,
            error_code,
            error_message
        )
        VALUES (
            NEW.submission_id,
            NULL,
            NEW.status,
            NEW.error_code,
            NEW.error_message
        );
    ELSIF NEW.status IS DISTINCT FROM OLD.status THEN
        INSERT INTO transaction_submission_status_history (
            submission_id,
            previous_status,
            new_status,
            error_code,
            error_message
        )
        VALUES (
            NEW.submission_id,
            OLD.status,
            NEW.status,
            NEW.error_code,
            NEW.error_message
        );
    END IF;

    RETURN NEW;
END;
$$;

CREATE TRIGGER transaction_submissions_record_status_change
AFTER INSERT OR UPDATE OF status
ON transaction_submissions
FOR EACH ROW
EXECUTE FUNCTION record_transaction_submission_status_change();

CREATE TRIGGER transaction_submissions_set_updated_at
BEFORE UPDATE ON transaction_submissions
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();


-- Lightweight PostgreSQL-backed queue. Workers claim due jobs using:
--
-- SELECT ...
-- FOR UPDATE SKIP LOCKED
--
-- This allows multiple workers without two workers processing the same row.
CREATE TABLE jobs (
    job_id BIGINT GENERATED ALWAYS AS IDENTITY,

    organization_id NUMERIC(20, 0),
    submission_id BIGINT,

    job_type TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::JSONB,

    -- Optional application-level deduplication key.
    idempotency_key TEXT,

    status TEXT NOT NULL DEFAULT 'queued',

    priority INTEGER NOT NULL DEFAULT 100,
    run_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    attempts INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 10,

    locked_at TIMESTAMPTZ,
    locked_by TEXT,
    lease_expires_at TIMESTAMPTZ,

    last_error TEXT,

    completed_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT jobs_pkey
        PRIMARY KEY (job_id),

    CONSTRAINT jobs_organization_fkey
        FOREIGN KEY (organization_id)
        REFERENCES organizations (organization_id)
        ON DELETE RESTRICT,

    CONSTRAINT jobs_submission_fkey
        FOREIGN KEY (submission_id)
        REFERENCES transaction_submissions (submission_id)
        ON DELETE RESTRICT,

    CONSTRAINT jobs_status_valid
        CHECK (
            status IN (
                'queued',
                'running',
                'succeeded',
                'dead',
                'cancelled'
            )
        ),

    CONSTRAINT jobs_type_length
        CHECK (
            char_length(job_type) BETWEEN 1 AND 120
        ),

    CONSTRAINT jobs_payload_object
        CHECK (
            jsonb_typeof(payload) = 'object'
        ),

    CONSTRAINT jobs_attempts_valid
        CHECK (
            attempts >= 0
            AND max_attempts > 0
            AND attempts <= max_attempts
        ),

    CONSTRAINT jobs_priority_valid
        CHECK (
            priority >= 0
        ),

    CONSTRAINT jobs_locked_by_length
        CHECK (
            locked_by IS NULL
            OR char_length(locked_by) BETWEEN 1 AND 200
        ),

    CONSTRAINT jobs_lease_valid
        CHECK (
            lease_expires_at IS NULL
            OR locked_at IS NOT NULL
        ),

    CONSTRAINT jobs_completion_valid
        CHECK (
            (
                status IN ('succeeded', 'dead', 'cancelled')
                AND completed_at IS NOT NULL
            )
            OR
            (
                status IN ('queued', 'running')
                AND completed_at IS NULL
            )
        )
);

CREATE UNIQUE INDEX jobs_idempotency_key_unique
    ON jobs (idempotency_key)
    WHERE idempotency_key IS NOT NULL;

CREATE INDEX jobs_claim_index
    ON jobs (
        priority,
        run_at,
        job_id
    )
    WHERE status = 'queued';

CREATE INDEX jobs_expired_lease_index
    ON jobs (
        lease_expires_at,
        job_id
    )
    WHERE status = 'running';

CREATE INDEX jobs_submission_index
    ON jobs (
        submission_id,
        created_at DESC
    )
    WHERE submission_id IS NOT NULL;

CREATE TRIGGER jobs_set_updated_at
BEFORE UPDATE ON jobs
FOR EACH ROW
EXECUTE FUNCTION set_row_updated_at();


COMMENT ON TABLE transaction_submissions IS
    'Durable transaction-attempt state, including RPC-uncertain submissions that must be reconciled rather than blindly retried.';

COMMENT ON TABLE transaction_submission_status_history IS
    'Append-only audit history of transaction submission state changes.';

COMMENT ON TABLE jobs IS
    'PostgreSQL-backed durable job queue for transaction reconciliation, event indexing, account synchronization and vault-balance refreshes.';
