DROP TABLE IF EXISTS approval_policy_version_chain_members;
DROP TABLE IF EXISTS approval_policy_version_chain_accounts;
DROP TABLE IF EXISTS vault_chain_accounts;
DROP TABLE IF EXISTS recipient_chain_accounts;
DROP TABLE IF EXISTS member_wallet_chain_accounts;
DROP TABLE IF EXISTS member_chain_accounts;

ALTER TABLE organization_chain_accounts
DROP CONSTRAINT IF EXISTS
    organization_chain_accounts_identity_with_pda_unique;
