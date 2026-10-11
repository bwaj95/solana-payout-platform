DROP TRIGGER IF EXISTS
    transaction_submissions_record_status_change
    ON transaction_submissions;

DROP FUNCTION IF EXISTS
    record_transaction_submission_status_change();

DROP TABLE IF EXISTS jobs;
DROP TABLE IF EXISTS transaction_submission_status_history;
DROP TABLE IF EXISTS transaction_submissions;
