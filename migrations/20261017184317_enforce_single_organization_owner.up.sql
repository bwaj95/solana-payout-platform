-- Add up migration script here
-- Only rows carrying ROLE_OWNER participate in this index.
-- ROLE_OWNER is bit 0, so its numeric value is 1.
--
-- PostgreSQL then permits only one such row per organization_id,
-- including when concurrent requests attempt to create owners.
CREATE UNIQUE INDEX organization_members_single_owner_unique
    ON organization_members (organization_id)
    WHERE (requested_roles & 1) <> 0;

COMMENT ON INDEX organization_members_single_owner_unique IS
    'Guarantees that each application organization has at most one member carrying ROLE_OWNER.';