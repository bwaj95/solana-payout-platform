DROP TRIGGER IF EXISTS organizations_set_updated_at
    ON organizations;

DROP TABLE IF EXISTS vaults;
DROP TABLE IF EXISTS recipients;
DROP TABLE IF EXISTS organization_members;

DROP FUNCTION IF EXISTS set_row_updated_at();

DROP SEQUENCE IF EXISTS vaults_vault_id_seq;
DROP SEQUENCE IF EXISTS recipients_recipient_id_seq;
DROP SEQUENCE IF EXISTS organization_members_member_id_seq;
