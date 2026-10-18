CREATE TABLE wallet_auth_challenges (
    challenge_id UUID NOT NULL DEFAULT gen_random_uuid(),
    wallet_address TEXT NOT NULL,
    message TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT wallet_auth_challenges_pkey
        PRIMARY KEY (challenge_id),

    CONSTRAINT wallet_auth_challenges_wallet_length
        CHECK (
            char_length(wallet_address) >= 32
            AND char_length(wallet_address) <= 44
        ),

    CONSTRAINT wallet_auth_challenges_message_not_empty
        CHECK (char_length(btrim(message)) > 0),

    CONSTRAINT wallet_auth_challenges_expiry_after_creation
        CHECK (expires_at > created_at),

    CONSTRAINT wallet_auth_challenges_consumed_after_creation
        CHECK (
            consumed_at IS NULL
            OR consumed_at >= created_at
        )
);

CREATE INDEX wallet_auth_challenges_wallet_index
    ON wallet_auth_challenges (
        wallet_address,
        created_at DESC
    );

CREATE INDEX wallet_auth_challenges_expiry_index
    ON wallet_auth_challenges (expires_at)
    WHERE consumed_at IS NULL;


CREATE TABLE wallet_sessions (
    session_id UUID NOT NULL DEFAULT gen_random_uuid(),
    token_hash BYTEA NOT NULL,
    wallet_address TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    CONSTRAINT wallet_sessions_pkey
        PRIMARY KEY (session_id),

    CONSTRAINT wallet_sessions_token_hash_unique
        UNIQUE (token_hash),

    CONSTRAINT wallet_sessions_wallet_length
        CHECK (
            char_length(wallet_address) >= 32
            AND char_length(wallet_address) <= 44
        ),

    CONSTRAINT wallet_sessions_token_hash_length
        CHECK (octet_length(token_hash) = 32),

    CONSTRAINT wallet_sessions_expiry_after_creation
        CHECK (expires_at > created_at),

    CONSTRAINT wallet_sessions_last_seen_after_creation
        CHECK (last_seen_at >= created_at),

    CONSTRAINT wallet_sessions_revoked_after_creation
        CHECK (
            revoked_at IS NULL
            OR revoked_at >= created_at
        )
);

CREATE INDEX wallet_sessions_wallet_index
    ON wallet_sessions (
        wallet_address,
        created_at DESC
    );

CREATE INDEX wallet_sessions_active_expiry_index
    ON wallet_sessions (expires_at)
    WHERE revoked_at IS NULL;