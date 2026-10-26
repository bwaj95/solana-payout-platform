use std::{env, error::Error, time::Duration};

use payout_backend::jobs::{
    ClaimedJob, JobFailureDisposition, JobQueueError, claim_next_job, defer_job,
    mark_job_succeeded, record_job_failure, recover_expired_job_leases,
};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::test]
async fn due_job_can_be_claimed_only_once() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    /*
     * Everything in this test runs inside one transaction.
     *
     * Rolling it back removes the test job and its state changes, leaving
     * the development database unchanged.
     */
    let mut transaction = pool.begin().await?;

    /*
     * Use a unique job type so this test cannot accidentally claim one of
     * the real reconciliation jobs already queued in the database.
     */
    let job_type = format!("test_claim_job_{}", Uuid::new_v4());
    let idempotency_key = format!("test-job:{}", Uuid::new_v4());
    let payload = json!({
        "test": true,
    });

    let inserted_job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key
        )
        VALUES (
            $1,
            $2,
            $3
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        &idempotency_key,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let worker_id = format!("test-worker:{}", Uuid::new_v4());

    let claimed_job = claim_next_job(&mut transaction, &worker_id, &job_type, 30)
        .await?
        .expect("the inserted due job should be claimable");

    assert_eq!(claimed_job.job_id, inserted_job_id);
    assert_eq!(claimed_job.job_type, job_type);
    assert_eq!(claimed_job.attempts, 1);
    assert_eq!(claimed_job.max_attempts, 10);
    assert_eq!(claimed_job.locked_by, worker_id);
    assert!(claimed_job.lease_expires_at > claimed_job.locked_at);

    let stored_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_by,
            locked_at,
            lease_expires_at
        FROM jobs
        WHERE job_id = $1
        "#,
        inserted_job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(stored_job.status, "running");
    assert_eq!(stored_job.attempts, 1);
    assert_eq!(stored_job.locked_by.as_deref(), Some(worker_id.as_str()));
    assert!(stored_job.locked_at.is_some());
    assert!(stored_job.lease_expires_at.is_some());

    /*
     * The first claim changed the only matching job to running.
     * It must therefore not be returned by another claim.
     */
    let second_claim = claim_next_job(&mut transaction, &worker_id, &job_type, 30).await?;

    assert!(second_claim.is_none());

    mark_job_succeeded(&mut transaction, &claimed_job).await?;

    let completed_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        inserted_job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(completed_job.status, "succeeded");
    assert_eq!(completed_job.attempts, 1);
    assert!(completed_job.locked_at.is_none());
    assert!(completed_job.locked_by.is_none());
    assert!(completed_job.lease_expires_at.is_none());
    assert!(completed_job.completed_at.is_some());

    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn concurrent_workers_claim_different_jobs_without_waiting() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    /*
     * A unique job type isolates this test from real application jobs
     * and from another test running concurrently.
     */
    let test_id = Uuid::new_v4();
    let job_type = format!("test_concurrent_claim_{test_id}");
    let payload = json!({
        "test": true,
        "purpose": "concurrent job claiming",
    });

    /*
     * These jobs must be committed before opening the worker transactions.
     *
     * Separate transactions cannot see another transaction's uncommitted
     * inserts.
     */
    let mut setup_transaction = pool.begin().await?;
    let mut inserted_job_ids = Vec::new();

    for job_number in 1..=2 {
        let idempotency_key = format!("test-concurrent-job:{test_id}:{job_number}");

        let job_id = sqlx::query_scalar!(
            r#"
            INSERT INTO jobs (
                job_type,
                payload,
                idempotency_key
            )
            VALUES (
                $1,
                $2,
                $3
            )
            RETURNING job_id
            "#,
            &job_type,
            &payload,
            &idempotency_key,
        )
        .fetch_one(&mut *setup_transaction)
        .await?;

        inserted_job_ids.push(job_id);
    }

    setup_transaction.commit().await?;

    /*
     * These represent three independent workers. Each has its own
     * PostgreSQL transaction and therefore its own database connection.
     */
    let mut worker_a_transaction = pool.begin().await?;
    let mut worker_b_transaction = pool.begin().await?;
    let mut worker_c_transaction = pool.begin().await?;

    let worker_a_id = format!("worker-a:{test_id}");
    let worker_b_id = format!("worker-b:{test_id}");
    let worker_c_id = format!("worker-c:{test_id}");

    /*
     * Worker A claims the first job and deliberately keeps its transaction
     * open. Its row lock remains held.
     */
    let worker_a_result =
        claim_next_job(&mut worker_a_transaction, &worker_a_id, &job_type, 30).await;

    /*
     * Worker B should skip A's locked row and claim the second job.
     *
     * The timeout verifies that B does not sit waiting for A's lock.
     */
    let worker_b_result = tokio::time::timeout(
        Duration::from_secs(2),
        claim_next_job(&mut worker_b_transaction, &worker_b_id, &job_type, 30),
    )
    .await;

    /*
     * Both matching rows are now locked. Worker C should skip both and
     * return None without waiting.
     */
    let worker_c_result = tokio::time::timeout(
        Duration::from_secs(2),
        claim_next_job(&mut worker_c_transaction, &worker_c_id, &job_type, 30),
    )
    .await;

    /*
     * Release all locks before checking assertions or deleting fixtures.
     *
     * Rolling these transactions back also changes the claimed jobs back
     * to their previously committed queued state.
     */
    worker_a_transaction.rollback().await?;
    worker_b_transaction.rollback().await?;
    worker_c_transaction.rollback().await?;

    /*
     * Unlike the first test, the fixture inserts were committed so that
     * multiple transactions could see them. We therefore clean them up
     * explicitly.
     */
    sqlx::query!(
        r#"
        DELETE FROM jobs
        WHERE job_type = $1
        "#,
        &job_type,
    )
    .execute(&pool)
    .await?;

    /*
     * Perform the assertions after releasing locks and cleaning up.
     */
    let worker_a_job = worker_a_result?.expect("worker A should claim the first job");

    let worker_b_claim = worker_b_result?;
    let worker_b_job = worker_b_claim?.expect("worker B should skip job one and claim job two");

    let worker_c_claim = worker_c_result?;
    let worker_c_job = worker_c_claim?;

    assert_ne!(worker_a_job.job_id, worker_b_job.job_id);

    assert!(inserted_job_ids.contains(&worker_a_job.job_id));
    assert!(inserted_job_ids.contains(&worker_b_job.job_id));

    assert_eq!(worker_a_job.attempts, 1);
    assert_eq!(worker_b_job.attempts, 1);

    assert!(
        worker_c_job.is_none(),
        "worker C should find no unlocked matching job"
    );

    Ok(())
}

#[tokio::test]
async fn failure_requeues_until_attempt_budget_is_exhausted() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();
    let job_type = format!("test_retry_job_{test_id}");
    let idempotency_key = format!("test-retry-job:{test_id}");
    let payload = json!({
        "test": true,
        "purpose": "retry budget",
    });

    let job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            2
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        &idempotency_key,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let worker_id = format!("test-worker:{test_id}");

    /*
     * First claim: attempts becomes 1.
     */
    let first_attempt = claim_next_job(&mut transaction, &worker_id, &job_type, 30)
        .await?
        .expect("first attempt should claim the job");

    assert_eq!(first_attempt.attempts, 1);

    /*
     * Because attempt 1 is below max_attempts 2, a retryable failure
     * returns the job to queued. Delay zero makes it immediately due.
     */
    let first_disposition =
        record_job_failure(&mut transaction, &first_attempt, 0, "temporary RPC failure").await?;

    assert_eq!(first_disposition, JobFailureDisposition::Requeued);

    let requeued_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_by,
            lease_expires_at,
            completed_at,
            last_error
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(requeued_job.status, "queued");
    assert_eq!(requeued_job.attempts, 1);
    assert!(requeued_job.locked_by.is_none());
    assert!(requeued_job.lease_expires_at.is_none());
    assert!(requeued_job.completed_at.is_none());
    assert_eq!(
        requeued_job.last_error.as_deref(),
        Some("temporary RPC failure")
    );

    /*
     * Second claim: attempts becomes 2.
     */
    let second_attempt = claim_next_job(&mut transaction, &worker_id, &job_type, 30)
        .await?
        .expect("second attempt should reclaim the queued job");

    assert_eq!(second_attempt.attempts, 2);

    /*
     * Attempt 2 equals max_attempts, so another failure permanently
     * dead-letters the job instead of requeueing it forever.
     */
    let second_disposition = record_job_failure(
        &mut transaction,
        &second_attempt,
        0,
        "RPC failure after final attempt",
    )
    .await?;

    assert_eq!(second_disposition, JobFailureDisposition::Dead);

    let dead_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at,
            last_error
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(dead_job.status, "dead");
    assert_eq!(dead_job.attempts, 2);
    assert!(dead_job.locked_at.is_none());
    assert!(dead_job.locked_by.is_none());
    assert!(dead_job.lease_expires_at.is_none());
    assert!(dead_job.completed_at.is_some());
    assert_eq!(
        dead_job.last_error.as_deref(),
        Some("RPC failure after final attempt")
    );

    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn stale_worker_cannot_complete_a_newer_attempt() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    /*
     * One transaction is sufficient here because this test is checking
     * attempt and worker fencing, rather than concurrent row locking.
     *
     * Rolling back at the end removes the complete test fixture.
     */
    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();
    let job_type = format!("test_stale_worker_{test_id}");
    let idempotency_key = format!("test-stale-worker:{test_id}");
    let payload = json!({
        "test": true,
        "purpose": "stale worker fencing"
    });

    let job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            3
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        &idempotency_key,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let worker_a_id = format!("worker-a:{test_id}");
    let worker_b_id = format!("worker-b:{test_id}");

    /*
     * Worker A receives attempt 1.
     *
     * The returned ClaimedJob is Worker A's ownership token. It records:
     * - the job ID;
     * - attempt number 1;
     * - Worker A's identity;
     * - the lease expiry.
     */
    let worker_a_attempt: ClaimedJob =
        claim_next_job(&mut transaction, &worker_a_id, &job_type, 30)
            .await?
            .expect("worker A should claim attempt 1");

    /*
     * Worker A reports a retryable failure.
     *
     * Because attempt 1 is below max_attempts 3, the job returns to
     * queued. A zero-second retry delay makes it immediately claimable.
     */

    let disposition = record_job_failure(
        &mut transaction,
        &worker_a_attempt,
        0,
        "temporary failure from worker A",
    )
    .await?;

    assert_eq!(disposition, JobFailureDisposition::Requeued);

    /*
     * Worker B now claims the same job.
     *
     * The job ID remains the same, but attempts becomes 2 and locked_by
     * becomes Worker B. This represents a new ownership generation.
     */
    let worker_b_attempt: ClaimedJob =
        claim_next_job(&mut transaction, &worker_b_id, &job_type, 30)
            .await?
            .expect("worker B should claim attempt 2");

    assert_eq!(worker_b_attempt.job_id, job_id);
    assert_eq!(worker_b_attempt.attempts, 2);
    assert_eq!(worker_b_attempt.locked_by, worker_b_id);

    /*
     * Imagine Worker A's original work did not actually stop immediately.
     * Its delayed result now arrives and tries to mark the job successful.
     *
     * mark_job_succeeded includes these fencing conditions:
     *
     *     attempts = worker_a_attempt.attempts
     *     locked_by = worker_a_attempt.locked_by
     *
     * The database row now belongs to Worker B at attempt 2, so Worker A's
     * conditional UPDATE must match zero rows and return LeaseLost.
     */
    let stale_completion = mark_job_succeeded(&mut transaction, &worker_a_attempt).await;

    match stale_completion {
        Err(JobQueueError::LeaseLost {
            job_id: lost_job_id,
            worker_id,
            attempt,
        }) => {
            assert_eq!(lost_job_id, job_id);
            assert_eq!(worker_id, worker_a_id);
            assert_eq!(attempt, 1);
        }

        Ok(()) => {
            panic!("stale worker A must not complete worker B's attempt");
        }

        Err(other) => {
            panic!("expected LeaseLost, received {other:?}");
        }
    }

    /*
     * Worker A's rejected update must not modify Worker B's active claim.
     */
    let running_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_by,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(running_job.status, "running");
    assert_eq!(running_job.attempts, 2);
    assert_eq!(running_job.locked_by.as_deref(), Some(worker_b_id.as_str()));
    assert!(running_job.completed_at.is_none());

    /*
     * Worker B still owns the current attempt and can complete it.
     */
    mark_job_succeeded(&mut transaction, &worker_b_attempt).await?;

    let completed_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(completed_job.status, "succeeded");
    assert_eq!(completed_job.attempts, 2);
    assert!(completed_job.locked_at.is_none());
    assert!(completed_job.locked_by.is_none());
    assert!(completed_job.lease_expires_at.is_none());
    assert!(completed_job.completed_at.is_some());

    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn expired_leases_are_requeued_or_dead_lettered() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();
    let job_type = format!("test_expired_lease_{test_id}");

    let payload = json!({
        "test": true,
        "purpose": "expired lease recovery",
    });

    /*
     * This job has two allowed attempts.
     *
     * After its first worker dies, one attempt still remains, so recovery
     * should return it to queued.
     */
    let retryable_job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            2
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        format!("test-expired-retryable:{test_id}"),
    )
    .fetch_one(&mut *transaction)
    .await?;

    /*
     * This job permits only one attempt.
     *
     * If its first worker dies, its complete attempt budget has been
     * consumed, so recovery should move it to dead.
     */
    let exhausted_job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            1
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        format!("test-expired-exhausted:{test_id}"),
    )
    .fetch_one(&mut *transaction)
    .await?;

    let retryable_worker_id = format!("retryable-worker:{test_id}");
    let exhausted_worker_id = format!("exhausted-worker:{test_id}");

    let retryable_attempt = claim_next_job(&mut transaction, &retryable_worker_id, &job_type, 30)
        .await?
        .expect("the retryable job should be claimed");

    let exhausted_attempt = claim_next_job(&mut transaction, &exhausted_worker_id, &job_type, 30)
        .await?
        .expect("the exhausted job should be claimed");

    assert_eq!(retryable_attempt.job_id, retryable_job_id);
    assert_eq!(retryable_attempt.attempts, 1);

    assert_eq!(exhausted_attempt.job_id, exhausted_job_id);
    assert_eq!(exhausted_attempt.attempts, 1);

    /*
     * Waiting for a real 30-second lease would make the test slow.
     *
     * Instead, move both lease timestamps into the past to simulate
     * workers that claimed their jobs and then crashed.
     */
    sqlx::query!(
        r#"
        UPDATE jobs
        SET lease_expires_at = now() - INTERVAL '1 second'
        WHERE job_id = $1
           OR job_id = $2
        "#,
        retryable_job_id,
        exhausted_job_id,
    )
    .execute(&mut *transaction)
    .await?;

    /*
     * Recover at most 10 expired jobs.
     *
     * Retry delay zero means the retryable job becomes immediately due.
     */
    let summary = recover_expired_job_leases(&mut transaction, 10, 0).await?;

    assert_eq!(summary.requeued, 1);
    assert_eq!(summary.dead, 1);

    let retryable_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at,
            last_error
        FROM jobs
        WHERE job_id = $1
        "#,
        retryable_job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(retryable_job.status, "queued");
    assert_eq!(retryable_job.attempts, 1);
    assert!(retryable_job.locked_at.is_none());
    assert!(retryable_job.locked_by.is_none());
    assert!(retryable_job.lease_expires_at.is_none());
    assert!(retryable_job.completed_at.is_none());
    assert_eq!(
        retryable_job.last_error.as_deref(),
        Some("worker lease expired before completion")
    );

    let exhausted_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at,
            last_error
        FROM jobs
        WHERE job_id = $1
        "#,
        exhausted_job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(exhausted_job.status, "dead");
    assert_eq!(exhausted_job.attempts, 1);
    assert!(exhausted_job.locked_at.is_none());
    assert!(exhausted_job.locked_by.is_none());
    assert!(exhausted_job.lease_expires_at.is_none());
    assert!(exhausted_job.completed_at.is_some());
    assert_eq!(
        exhausted_job.last_error.as_deref(),
        Some("worker lease expired after final attempt")
    );

    /*
     * The crashed worker's old ownership object still exists in memory,
     * but its lease is no longer valid and the database row is no longer
     * owned by it.
     *
     * Therefore, it must not be allowed to report success.
     */
    let stale_completion = mark_job_succeeded(&mut transaction, &retryable_attempt).await;

    assert!(matches!(
        stale_completion,
        Err(JobQueueError::LeaseLost {
            job_id,
            attempt: 1,
            ..
        }) if job_id == retryable_job_id
    ));

    /*
     * A new worker can claim the recovered job as attempt 2.
     */
    let replacement_worker_id = format!("replacement-worker:{test_id}");

    let replacement_attempt =
        claim_next_job(&mut transaction, &replacement_worker_id, &job_type, 30)
            .await?
            .expect("the recovered retryable job should be claimable again");

    assert_eq!(replacement_attempt.job_id, retryable_job_id);
    assert_eq!(replacement_attempt.attempts, 2);
    assert_eq!(replacement_attempt.locked_by, replacement_worker_id);

    mark_job_succeeded(&mut transaction, &replacement_attempt).await?;

    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn deferred_job_is_not_claimable_until_run_at() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();
    let job_type = format!("test_defer_job_{test_id}");
    let idempotency_key = format!("test-defer-job:{test_id}");
    let previous_error = "previous temporary RPC error";

    let payload = json!({
        "test": true,
        "purpose": "normal job deferral",
    });

    let job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts,
            last_error
        )
        VALUES (
            $1,
            $2,
            $3,
            3,
            $4
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        &idempotency_key,
        previous_error,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let first_worker_id = format!("first-worker:{test_id}");

    let first_attempt = claim_next_job(&mut transaction, &first_worker_id, &job_type, 30)
        .await?
        .expect("the inserted job should be claimable");

    assert_eq!(first_attempt.job_id, job_id);
    assert_eq!(first_attempt.attempts, 1);

    /*
     * The worker successfully checked the external operation, but it has
     * not reached a terminal state. Defer the next check for 30 seconds.
     */
    defer_job(&mut transaction, &first_attempt, 30).await?;

    let deferred_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            run_at > now() AS "scheduled_for_future!",
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at,
            last_error
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(deferred_job.status, "queued");
    assert_eq!(deferred_job.attempts, 1);
    assert!(deferred_job.scheduled_for_future);
    assert!(deferred_job.locked_at.is_none());
    assert!(deferred_job.locked_by.is_none());
    assert!(deferred_job.lease_expires_at.is_none());
    assert!(deferred_job.completed_at.is_none());

    /*
     * Deferral is not a processing failure, so it must not overwrite the
     * previously stored diagnostic error.
     */
    assert_eq!(deferred_job.last_error.as_deref(), Some(previous_error));

    /*
     * Although the job is queued, run_at is still in the future.
     * claim_next_job must therefore ignore it.
     */
    let early_claim = claim_next_job(&mut transaction, &first_worker_id, &job_type, 30).await?;

    assert!(
        early_claim.is_none(),
        "a deferred job must not be claimed before run_at"
    );

    /*
     * Simulate the deferral period passing without making the test sleep.
     */
    sqlx::query!(
        r#"
        UPDATE jobs
        SET run_at = now()
        WHERE job_id = $1
        "#,
        job_id,
    )
    .execute(&mut *transaction)
    .await?;

    let second_worker_id = format!("second-worker:{test_id}");

    let second_attempt = claim_next_job(&mut transaction, &second_worker_id, &job_type, 30)
        .await?
        .expect("the job should be claimable after run_at");

    assert_eq!(second_attempt.job_id, job_id);
    assert_eq!(second_attempt.attempts, 2);
    assert_eq!(second_attempt.locked_by, second_worker_id);

    mark_job_succeeded(&mut transaction, &second_attempt).await?;

    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn stale_attempt_from_same_worker_cannot_defer_newer_attempt() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();
    let job_type = format!("test_same_worker_stale_defer_{test_id}");
    let idempotency_key = format!("test-same-worker-stale-defer:{test_id}");

    let payload = json!({
        "test": true,
        "purpose": "same worker stale-attempt fencing",
    });

    let job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            3
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        &idempotency_key,
    )
    .fetch_one(&mut *transaction)
    .await?;

    /*
     * A worker ID belongs to the process lifetime. Therefore, the same
     * worker process can legitimately claim multiple attempts of one job.
     */
    let worker_id = format!("same-worker:{test_id}");

    let first_attempt = claim_next_job(&mut transaction, &worker_id, &job_type, 30)
        .await?
        .expect("the worker should claim attempt 1");

    assert_eq!(first_attempt.attempts, 1);

    /*
     * Zero-second deferral makes the job immediately due again.
     */
    defer_job(&mut transaction, &first_attempt, 0).await?;

    let second_attempt = claim_next_job(&mut transaction, &worker_id, &job_type, 30)
        .await?
        .expect("the same worker should claim attempt 2");

    assert_eq!(second_attempt.job_id, job_id);
    assert_eq!(second_attempt.attempts, 2);
    assert_eq!(second_attempt.locked_by, worker_id);

    /*
     * A delayed operation from attempt 1 now tries to defer the job.
     *
     * job_id and locked_by both match attempt 2, so only the attempts
     * fencing condition prevents this stale update.
     */
    let stale_result = defer_job(&mut transaction, &first_attempt, 0).await;

    assert!(matches!(
        stale_result,
        Err(JobQueueError::LeaseLost {
            job_id: lost_job_id,
            attempt: 1,
            ..
        }) if lost_job_id == job_id
    ));

    /*
     * The rejected stale update must not disturb attempt 2.
     */
    let running_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_by,
            locked_at,
            lease_expires_at
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(running_job.status, "running");
    assert_eq!(running_job.attempts, 2);
    assert_eq!(running_job.locked_by.as_deref(), Some(worker_id.as_str()));
    assert!(running_job.locked_at.is_some());
    assert!(running_job.lease_expires_at.is_some());

    mark_job_succeeded(&mut transaction, &second_attempt).await?;

    transaction.rollback().await?;

    Ok(())
}

#[tokio::test]
async fn final_attempt_cannot_be_deferred() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;

    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&database_url)
        .await?;

    let mut transaction = pool.begin().await?;

    let test_id = Uuid::new_v4();
    let job_type = format!("test_deferral_budget_{test_id}");
    let idempotency_key = format!("test-deferral-budget:{test_id}");

    let payload = json!({
        "test": true,
        "purpose": "deferral attempt-budget protection",
    });

    let job_id = sqlx::query_scalar!(
        r#"
        INSERT INTO jobs (
            job_type,
            payload,
            idempotency_key,
            max_attempts
        )
        VALUES (
            $1,
            $2,
            $3,
            1
        )
        RETURNING job_id
        "#,
        &job_type,
        &payload,
        &idempotency_key,
    )
    .fetch_one(&mut *transaction)
    .await?;

    let worker_id = format!("budget-worker:{test_id}");

    let final_attempt = claim_next_job(&mut transaction, &worker_id, &job_type, 30)
        .await?
        .expect("the worker should claim the only allowed attempt");

    assert_eq!(final_attempt.job_id, job_id);
    assert_eq!(final_attempt.attempts, 1);
    assert_eq!(final_attempt.max_attempts, 1);

    /*
     * Returning this job to queued would create an unclaimable row:
     *
     *     status = queued
     *     attempts = max_attempts
     *
     * claim_next_job requires attempts < max_attempts, so defer_job must
     * reject this transition.
     */
    let defer_result = defer_job(&mut transaction, &final_attempt, 5).await;

    assert!(matches!(
        defer_result,
        Err(JobQueueError::DeferralBudgetExhausted {
            job_id: exhausted_job_id,
            attempt: 1,
            max_attempts: 1,
        }) if exhausted_job_id == job_id
    ));

    /*
     * The rejected deferral must leave the current ownership untouched.
     */
    let still_running_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(still_running_job.status, "running");
    assert_eq!(still_running_job.attempts, 1);
    assert_eq!(
        still_running_job.locked_by.as_deref(),
        Some(worker_id.as_str())
    );
    assert!(still_running_job.locked_at.is_some());
    assert!(still_running_job.lease_expires_at.is_some());
    assert!(still_running_job.completed_at.is_none());

    /*
     * The future reconciliation worker will convert the exhausted,
     * unresolved poll into a terminal operational failure.
     */
    let disposition = record_job_failure(
        &mut transaction,
        &final_attempt,
        0,
        "reconciliation poll budget exhausted",
    )
    .await?;

    assert_eq!(disposition, JobFailureDisposition::Dead);

    let dead_job = sqlx::query!(
        r#"
        SELECT
            status,
            attempts,
            locked_at,
            locked_by,
            lease_expires_at,
            completed_at,
            last_error
        FROM jobs
        WHERE job_id = $1
        "#,
        job_id,
    )
    .fetch_one(&mut *transaction)
    .await?;

    assert_eq!(dead_job.status, "dead");
    assert_eq!(dead_job.attempts, 1);
    assert!(dead_job.locked_at.is_none());
    assert!(dead_job.locked_by.is_none());
    assert!(dead_job.lease_expires_at.is_none());
    assert!(dead_job.completed_at.is_some());
    assert_eq!(
        dead_job.last_error.as_deref(),
        Some("reconciliation poll budget exhausted")
    );

    transaction.rollback().await?;

    Ok(())
}
