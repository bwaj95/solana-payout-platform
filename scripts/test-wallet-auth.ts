import { readFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { randomUUID } from "node:crypto";

import { Keypair } from "@solana/web3.js";
import bs58 from "bs58";
import nacl from "tweetnacl";

const API_BASE_URL = process.env.API_BASE_URL ?? "http://127.0.0.1:8080";

const WALLET_PATH =
  process.env.ANCHOR_WALLET ?? join(homedir(), ".config", "solana", "id.json");

type ChallengeResponse = {
  challengeId: string;
  walletAddress: string;
  message: string;
  expiresAt: string;
};

type SessionResponse = {
  sessionId: string;
  walletAddress: string;
  accessToken: string;
  tokenType: string;
  expiresAt: string;
};

type CurrentSessionResponse = {
  sessionId: string;
  walletAddress: string;
  expiresAt: string;
};

type ErrorResponse = {
  error?: {
    code?: string;
    message?: string;
  };
};

type OrganizationResponse = {
  organizationId: string;
  name: string;
  status: string;
  createdAt: string;
  updatedAt: string;
};

type OrganizationMemberResponse = {
  organizationId: string;
  memberId: string;
  displayName: string;
  externalReference: string | null;
  requestedAuthorizedWallet: string;
  requestedRoles: number;
  lifecycleStatus: string;
  createdAt: string;
  updatedAt: string;
};

type OrganizationProvisioningResponse = {
  organization: OrganizationResponse;
  ownerMember: OrganizationMemberResponse;
};

type InitializationSubmissionRequest = {
  idempotencyKey: string;
  transactionSignature: string;
  lastValidBlockHeight: number;
  submissionStatus: "submitted" | "unknown";
};

type InitializationSubmissionResponse = {
  submissionId: number;
  idempotencyKey: string;

  organizationId: string;
  ownerMemberId: string;

  organizationStatus: string;
  ownerMemberStatus: string;

  transactionSignature: string;
  lastValidBlockHeight: number;
  submissionStatus: string;

  createdAt: string;
  updatedAt: string;
};

async function loadKeypair(): Promise<Keypair> {
  const contents = await readFile(WALLET_PATH, "utf8");
  const secretKeyNumbers: unknown = JSON.parse(contents);

  if (
    !Array.isArray(secretKeyNumbers) ||
    secretKeyNumbers.length !== 64 ||
    !secretKeyNumbers.every(
      (value) => Number.isInteger(value) && value >= 0 && value <= 255
    )
  ) {
    throw new Error(`Expected a 64-byte Solana keypair in ${WALLET_PATH}`);
  }

  return Keypair.fromSecretKey(Uint8Array.from(secretKeyNumbers));
}

async function requestChallenge(
  walletAddress: string
): Promise<ChallengeResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/challenge`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({
      walletAddress,
    }),
  });

  const body = (await response.json()) as ChallengeResponse | ErrorResponse;

  if (!response.ok) {
    throw new Error(
      `Challenge request failed (${response.status}): ${JSON.stringify(body)}`
    );
  }

  return body as ChallengeResponse;
}

async function verifyChallenge(input: {
  challengeId: string;
  walletAddress: string;
  signature: string;
}): Promise<SessionResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/verify`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify(input),
  });

  const body = (await response.json()) as SessionResponse | ErrorResponse;

  if (!response.ok) {
    throw new Error(
      `Challenge verification failed (${response.status}): ${JSON.stringify(
        body
      )}`
    );
  }

  return body as SessionResponse;
}

async function verifyReplayIsRejected(input: {
  challengeId: string;
  walletAddress: string;
  signature: string;
}): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/verify`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify(input),
  });

  const body = (await response.json()) as ErrorResponse;

  if (response.status !== 409) {
    throw new Error(
      `Expected replay to return 409, received ${
        response.status
      }: ${JSON.stringify(body)}`
    );
  }

  if (body.error?.code !== "authentication_challenge_consumed") {
    throw new Error(`Unexpected replay error: ${JSON.stringify(body)}`);
  }

  console.log(`Replay rejected: ${response.status} ${body.error.code}`);
}

async function getCurrentSession(
  accessToken: string
): Promise<CurrentSessionResponse> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/me`, {
    method: "GET",
    headers: {
      authorization: `Bearer ${accessToken}`,
    },
  });

  const body = (await response.json()) as
    | CurrentSessionResponse
    | ErrorResponse;

  if (!response.ok) {
    throw new Error(
      `Authenticated request failed (${response.status}): ${JSON.stringify(
        body
      )}`
    );
  }

  return body as CurrentSessionResponse;
}

async function verifyInvalidTokenIsRejected(): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/v1/auth/me`, {
    method: "GET",
    headers: {
      authorization: "Bearer this-is-not-a-valid-session-token",
    },
  });

  const body = (await response.json()) as ErrorResponse;

  if (response.status !== 401) {
    throw new Error(
      `Expected invalid token to return 401, received ${
        response.status
      }: ${JSON.stringify(body)}`
    );
  }

  if (body.error?.code !== "invalid_authentication_token") {
    throw new Error(
      `Unexpected invalid-token response: ${JSON.stringify(body)}`
    );
  }

  console.log(`Invalid token rejected: ${response.status} ${body.error.code}`);
}

async function createOrganization(
  accessToken: string
): Promise<OrganizationProvisioningResponse> {
  const uniqueSuffix = Date.now().toString();

  const response = await fetch(`${API_BASE_URL}/api/v1/organizations`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${accessToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({
      name: `Demo Treasury ${uniqueSuffix}`,
      ownerDisplayName: "Demo Owner",
      ownerExternalReference: `demo-owner-${uniqueSuffix}`,
    }),
  });

  const body = (await response.json()) as
    | OrganizationProvisioningResponse
    | ErrorResponse;

  if (response.status !== 201) {
    throw new Error(
      `Organization provisioning failed (${response.status}): ${JSON.stringify(
        body
      )}`
    );
  }

  return body as OrganizationProvisioningResponse;
}

async function verifyUnauthenticatedOrganizationCreationIsRejected(): Promise<void> {
  const response = await fetch(`${API_BASE_URL}/api/v1/organizations`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
    },
    body: JSON.stringify({
      name: "Unauthorized Organization",
      ownerDisplayName: "Unauthorized Owner",
    }),
  });

  const body = (await response.json()) as ErrorResponse;

  if (response.status !== 401) {
    throw new Error(
      `Expected unauthenticated organization creation to return 401, received ${
        response.status
      }: ${JSON.stringify(body)}`
    );
  }

  if (body.error?.code !== "missing_authentication_token") {
    throw new Error(
      `Unexpected unauthenticated response: ${JSON.stringify(body)}`
    );
  }

  console.log(
    `Unauthenticated creation rejected: ${response.status} ${body.error.code}`
  );
}

async function recordInitializationSubmission(input: {
  accessToken: string;
  organizationId: string;
  request: InitializationSubmissionRequest;
}): Promise<InitializationSubmissionResponse> {
  const response = await fetch(
    `${API_BASE_URL}/api/v1/organizations/${input.organizationId}/initialization-submissions`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${input.accessToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify(input.request),
    }
  );

  const body = (await response.json()) as
    | InitializationSubmissionResponse
    | ErrorResponse;

  if (response.status !== 202) {
    throw new Error(
      `Initialization submission failed (${response.status}): ${JSON.stringify(
        body
      )}`
    );
  }

  return body as InitializationSubmissionResponse;
}

async function expectInitializationSubmissionError(input: {
  accessToken?: string;
  organizationId: string;
  request: InitializationSubmissionRequest;
  expectedStatus: number;
  expectedCode: string;
}): Promise<void> {
  const headers: Record<string, string> = {
    "content-type": "application/json",
  };

  if (input.accessToken !== undefined) {
    headers.authorization = `Bearer ${input.accessToken}`;
  }

  const response = await fetch(
    `${API_BASE_URL}/api/v1/organizations/${input.organizationId}/initialization-submissions`,
    {
      method: "POST",
      headers,
      body: JSON.stringify(input.request),
    }
  );

  const body = (await response.json()) as ErrorResponse;

  if (response.status !== input.expectedStatus) {
    throw new Error(
      `Expected status ${input.expectedStatus}, received ${
        response.status
      }: ${JSON.stringify(body)}`
    );
  }

  if (body.error?.code !== input.expectedCode) {
    throw new Error(
      `Expected error ${input.expectedCode}, received: ${JSON.stringify(body)}`
    );
  }

  console.log(`Rejected as expected: ${response.status} ${body.error.code}`);
}

async function main(): Promise<void> {
  const keypair = await loadKeypair();
  const walletAddress = keypair.publicKey.toBase58();

  console.log(`API: ${API_BASE_URL}`);
  console.log(`Wallet: ${walletAddress}`);

  const challenge = await requestChallenge(walletAddress);

  console.log(`Challenge: ${challenge.challengeId}`);
  console.log(`Expires at: ${challenge.expiresAt}`);

  /*
   * TextEncoder converts the exact JavaScript string returned by the API
   * into its UTF-8 bytes.
   *
   * These must be the same bytes verified by Rust using:
   * challenge.message.as_bytes()
   */
  const messageBytes = new TextEncoder().encode(challenge.message);

  /*
   * sign.detached creates a 64-byte Ed25519 signature without including
   * the original message in the returned value.
   */
  const signatureBytes = nacl.sign.detached(messageBytes, keypair.secretKey);

  const encodedSignature = bs58.encode(signatureBytes);

  const verificationRequest = {
    challengeId: challenge.challengeId,
    walletAddress,
    signature: encodedSignature,
  };

  const session = await verifyChallenge(verificationRequest);

  if (session.walletAddress !== walletAddress) {
    throw new Error("Session wallet does not match the signing wallet");
  }

  if (!session.accessToken) {
    throw new Error("API did not return an access token");
  }

  /*
   * Do not print the complete bearer token. Anyone possessing it could
   * use the session until it expires or is revoked.
   */
  const tokenPreview = `${session.accessToken.slice(0, 8)}...`;

  console.log(`Session: ${session.sessionId}`);
  console.log(`Token type: ${session.tokenType}`);
  console.log(`Access token: ${tokenPreview}`);
  console.log(`Session expires at: ${session.expiresAt}`);
  console.log("WALLET AUTHENTICATION PASSED");

  const currentSession = await getCurrentSession(session.accessToken);

  if (currentSession.sessionId !== session.sessionId) {
    throw new Error("Protected endpoint returned a different session");
  }

  if (currentSession.walletAddress !== walletAddress) {
    throw new Error("Protected endpoint returned a different wallet");
  }

  console.log(`Authenticated wallet: ${currentSession.walletAddress}`);
  console.log("AUTHENTICATED SESSION REQUEST PASSED");

  await verifyInvalidTokenIsRejected();

  console.log("INVALID SESSION TOKEN REJECTION PASSED");

  await verifyUnauthenticatedOrganizationCreationIsRejected();

  console.log("UNAUTHENTICATED ORGANIZATION CREATION REJECTION PASSED");

  const provisioning = await createOrganization(session.accessToken);

  if (
    provisioning.ownerMember.organizationId !==
    provisioning.organization.organizationId
  ) {
    throw new Error("Owner member belongs to a different organization");
  }

  if (provisioning.ownerMember.requestedAuthorizedWallet !== walletAddress) {
    throw new Error(
      "Owner member wallet does not match the authenticated wallet"
    );
  }

  if (provisioning.ownerMember.requestedRoles !== 3) {
    throw new Error(
      `Expected initial owner roles to be 3, received ${provisioning.ownerMember.requestedRoles}`
    );
  }

  if (
    provisioning.organization.status !== "draft" ||
    provisioning.ownerMember.lifecycleStatus !== "draft"
  ) {
    throw new Error("Organization and owner member must initially be drafts");
  }

  console.log(`Organization ID: ${provisioning.organization.organizationId}`);
  console.log(`Organization status: ${provisioning.organization.status}`);
  console.log(`Owner member ID: ${provisioning.ownerMember.memberId}`);
  console.log(
    `Owner wallet: ${provisioning.ownerMember.requestedAuthorizedWallet}`
  );
  console.log(`Owner roles: ${provisioning.ownerMember.requestedRoles}`);
  console.log("DATABASE ORGANIZATION CREATION PASSED");

  /*
   * This is structurally a Solana signature because it decodes to 64 bytes.
   * It has not been submitted to devnet. At this checkpoint we are testing
   * submission recording, idempotency and database state transitions.
   */
  const transactionSignature = bs58.encode(nacl.randomBytes(64));

  const initializationRequest: InitializationSubmissionRequest = {
    idempotencyKey: randomUUID(),
    transactionSignature,
    lastValidBlockHeight: 1_000_000_000,
    submissionStatus: "submitted",
  };

  await expectInitializationSubmissionError({
    organizationId: provisioning.organization.organizationId,
    request: initializationRequest,
    expectedStatus: 401,
    expectedCode: "missing_authentication_token",
  });

  console.log("UNAUTHENTICATED INITIALIZATION REJECTION PASSED");

  await expectInitializationSubmissionError({
    accessToken: session.accessToken,
    organizationId: provisioning.organization.organizationId,
    request: {
      ...initializationRequest,
      idempotencyKey: randomUUID(),
      transactionSignature: "not-a-solana-signature",
    },
    expectedStatus: 400,
    expectedCode: "invalid_transaction_signature",
  });

  console.log("INVALID TRANSACTION SIGNATURE REJECTION PASSED");

  const firstSubmission = await recordInitializationSubmission({
    accessToken: session.accessToken,
    organizationId: provisioning.organization.organizationId,
    request: initializationRequest,
  });

  if (
    firstSubmission.organizationId !== provisioning.organization.organizationId
  ) {
    throw new Error("Submission belongs to a different organization");
  }

  if (firstSubmission.ownerMemberId !== provisioning.ownerMember.memberId) {
    throw new Error("Submission references a different owner member");
  }

  if (
    firstSubmission.organizationStatus !== "provisioning" ||
    firstSubmission.ownerMemberStatus !== "provisioning"
  ) {
    throw new Error("Organization and owner must transition to provisioning");
  }

  if (firstSubmission.submissionStatus !== "submitted") {
    throw new Error(
      `Expected submitted status, received ${firstSubmission.submissionStatus}`
    );
  }

  console.log(`Submission ID: ${firstSubmission.submissionId}`);
  console.log(`Idempotency key: ${firstSubmission.idempotencyKey}`);
  console.log(`Transaction signature: ${transactionSignature}`);
  console.log("INITIALIZATION SUBMISSION RECORDING PASSED");

  /*
   * Send exactly the same logical request again. This represents an HTTP
   * retry after the client was unsure whether it received the first response.
   */
  const retriedSubmission = await recordInitializationSubmission({
    accessToken: session.accessToken,
    organizationId: provisioning.organization.organizationId,
    request: initializationRequest,
  });

  if (retriedSubmission.submissionId !== firstSubmission.submissionId) {
    throw new Error("Idempotent retry created a different submission");
  }

  if (retriedSubmission.createdAt !== firstSubmission.createdAt) {
    throw new Error("Idempotent retry created a new database row");
  }

  console.log(
    `Retry returned original submission: ${retriedSubmission.submissionId}`
  );
  console.log("IDEMPOTENT INITIALIZATION RETRY PASSED");

  /*
   * Reusing the key with different input is not a retry. It is an attempt
   * to give one idempotency key two different meanings.
   */
  await expectInitializationSubmissionError({
    accessToken: session.accessToken,
    organizationId: provisioning.organization.organizationId,
    request: {
      ...initializationRequest,
      lastValidBlockHeight: initializationRequest.lastValidBlockHeight + 1,
    },
    expectedStatus: 409,
    expectedCode: "idempotency_key_conflict",
  });

  console.log("IDEMPOTENCY KEY CONFLICT REJECTION PASSED");

  /*
   * A second draft organization attempts to report the same transaction
   * signature using a different idempotency key.
   */
  const secondOrganization = await createOrganization(session.accessToken);

  await expectInitializationSubmissionError({
    accessToken: session.accessToken,
    organizationId: secondOrganization.organization.organizationId,
    request: {
      ...initializationRequest,
      idempotencyKey: randomUUID(),
    },
    expectedStatus: 409,
    expectedCode: "transaction_signature_already_recorded",
  });

  console.log("DUPLICATE TRANSACTION SIGNATURE REJECTION PASSED");

  /*
   * Simulate two identical HTTP requests arriving at nearly the same time.
   *
   * Both requests use the same organization, idempotency key and transaction
   * details. The organization row lock serializes their state transitions.
   */
  const concurrentOrganization = await createOrganization(session.accessToken);

  const concurrentRequest: InitializationSubmissionRequest = {
    idempotencyKey: randomUUID(),
    transactionSignature: bs58.encode(nacl.randomBytes(64)),
    lastValidBlockHeight: 1_000_000_000,
    submissionStatus: "submitted",
  };

  const [concurrentResultOne, concurrentResultTwo] = await Promise.all([
    recordInitializationSubmission({
      accessToken: session.accessToken,
      organizationId: concurrentOrganization.organization.organizationId,
      request: concurrentRequest,
    }),

    recordInitializationSubmission({
      accessToken: session.accessToken,
      organizationId: concurrentOrganization.organization.organizationId,
      request: concurrentRequest,
    }),
  ]);

  if (concurrentResultOne.submissionId !== concurrentResultTwo.submissionId) {
    throw new Error(
      "Concurrent retries produced different transaction submissions"
    );
  }

  if (
    concurrentResultOne.idempotencyKey !== concurrentResultTwo.idempotencyKey
  ) {
    throw new Error("Concurrent retries returned different idempotency keys");
  }

  console.log(
    `Concurrent organization: ${concurrentOrganization.organization.organizationId}`
  );
  console.log(
    `Both requests returned submission: ${concurrentResultOne.submissionId}`
  );
  console.log("CONCURRENT IDEMPOTENT RETRY PASSED");

  /*
   * Submit the same signed challenge again. The database row is already
   * consumed, so the API must reject this replay attempt.
   */
  await verifyReplayIsRejected(verificationRequest);

  console.log("CHALLENGE REPLAY PROTECTION PASSED");
}

main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
