/**
 * Program IDL in camelCase format in order to be used in JS/TS.
 *
 * Note that this is only a type helper and is not the actual IDL. The original
 * IDL can be found at `target/idl/solana_payout_platform.json`.
 */
export type SolanaPayoutPlatform = {
  "address": "5SWH7YmBC7Tiri1MQLDbnWc1yTn3QqYBC1g9H3mZgbhv",
  "metadata": {
    "name": "solanaPayoutPlatform",
    "version": "0.1.0",
    "spec": "0.1.0",
    "description": "Created with Anchor"
  },
  "instructions": [
    {
      "name": "approvePayment",
      "discriminator": [
        21,
        123,
        195,
        139,
        107,
        141,
        34,
        187
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "approverMember",
            "vaultState",
            "recipient",
            "approvalPolicyVersion",
            "payment"
          ]
        },
        {
          "name": "approverMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "approver_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "vaultAta",
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "vaultState"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "recipient",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  99,
                  105,
                  112,
                  105,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "recipient.recipient_id",
                "account": "recipient"
              }
            ]
          }
        },
        {
          "name": "approvalPolicyVersion",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "approval_policy_version.policy_id",
                "account": "approvalPolicyVersion"
              },
              {
                "kind": "account",
                "path": "approval_policy_version.version",
                "account": "approvalPolicyVersion"
              }
            ]
          }
        },
        {
          "name": "payment",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  97,
                  121,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "payment.payment_id",
                "account": "payment"
              }
            ]
          }
        },
        {
          "name": "approval",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  97,
                  112,
                  112,
                  114,
                  111,
                  118,
                  97,
                  108
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "payment.payment_id",
                "account": "payment"
              },
              {
                "kind": "account",
                "path": "payment.payment_revision",
                "account": "payment"
              },
              {
                "kind": "account",
                "path": "approver_member.member_id",
                "account": "member"
              },
              {
                "kind": "account",
                "path": "approver_member.authorization_revision",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "paymentId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "cancelPayment",
      "discriminator": [
        217,
        129,
        71,
        37,
        216,
        193,
        38,
        33
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "adminMember",
            "vaultState",
            "payment"
          ]
        },
        {
          "name": "adminMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "admin_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "payment",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  97,
                  121,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "payment.payment_id",
                "account": "payment"
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "paymentId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "createMember",
      "discriminator": [
        49,
        46,
        45,
        241,
        122,
        143,
        136,
        73
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "adminMember"
          ]
        },
        {
          "name": "adminMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "admin_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "member",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "memberId"
              }
            ]
          }
        },
        {
          "name": "memberWallet",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114,
                  95,
                  119,
                  97,
                  108,
                  108,
                  101,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "authorizedWallet"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "memberId",
          "type": "u64"
        },
        {
          "name": "authorizedWallet",
          "type": "pubkey"
        },
        {
          "name": "roles",
          "type": "u16"
        }
      ]
    },
    {
      "name": "createPayment",
      "discriminator": [
        28,
        81,
        85,
        253,
        7,
        223,
        154,
        42
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "preparerMember",
            "recipient",
            "vaultState",
            "approvalPolicyVersion"
          ]
        },
        {
          "name": "preparerMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "preparer_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "recipient",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  99,
                  105,
                  112,
                  105,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "recipient.recipient_id",
                "account": "recipient"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "approvalPolicyVersion",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "approval_policy_version.policy_id",
                "account": "approvalPolicyVersion"
              },
              {
                "kind": "account",
                "path": "approval_policy_version.version",
                "account": "approvalPolicyVersion"
              }
            ]
          }
        },
        {
          "name": "payment",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  97,
                  121,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "paymentId"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "paymentId",
          "type": "u64"
        },
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "executeAfter",
          "type": "i64"
        }
      ]
    },
    {
      "name": "createPolicyVersion",
      "discriminator": [
        32,
        33,
        113,
        116,
        239,
        86,
        152,
        115
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "adminMember"
          ]
        },
        {
          "name": "adminMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "admin_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "approvalPolicyVersion",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "policyId"
              },
              {
                "kind": "arg",
                "path": "version"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "policyId",
          "type": "u64"
        },
        {
          "name": "version",
          "type": "u64"
        },
        {
          "name": "threshold",
          "type": "u8"
        }
      ]
    },
    {
      "name": "executeSplPayment",
      "discriminator": [
        123,
        191,
        66,
        237,
        163,
        25,
        68,
        172
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "executorMember",
            "vaultState",
            "recipient",
            "payment"
          ]
        },
        {
          "name": "executorMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "executor_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "vaultState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "vaultAta",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "vaultState"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "recipient",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  99,
                  105,
                  112,
                  105,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "recipient.recipient_id",
                "account": "recipient"
              }
            ]
          }
        },
        {
          "name": "destinationAta",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "recipient.current_destination",
                "account": "recipient"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "payment",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  97,
                  121,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "payment.payment_id",
                "account": "payment"
              }
            ]
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        }
      ],
      "args": [
        {
          "name": "paymentId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "finalizePaymentApproval",
      "discriminator": [
        58,
        30,
        248,
        172,
        129,
        44,
        37,
        246
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "finalizerMember",
            "vaultState",
            "recipient",
            "approvalPolicyVersion",
            "payment"
          ]
        },
        {
          "name": "finalizerMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "finalizer_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "vaultAta",
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "vaultState"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "recipient",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  99,
                  105,
                  112,
                  105,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "recipient.recipient_id",
                "account": "recipient"
              }
            ]
          }
        },
        {
          "name": "approvalPolicyVersion",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  111,
                  108,
                  105,
                  99,
                  121
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "approval_policy_version.policy_id",
                "account": "approvalPolicyVersion"
              },
              {
                "kind": "account",
                "path": "approval_policy_version.version",
                "account": "approvalPolicyVersion"
              }
            ]
          }
        },
        {
          "name": "payment",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  112,
                  97,
                  121,
                  109,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "payment.payment_id",
                "account": "payment"
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "paymentId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initializeOrganization",
      "discriminator": [
        21,
        20,
        253,
        138,
        250,
        160,
        119,
        87
      ],
      "accounts": [
        {
          "name": "creator",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "creator"
              },
              {
                "kind": "arg",
                "path": "organizationId"
              }
            ]
          }
        },
        {
          "name": "ownerMember",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "ownerMemberId"
              }
            ]
          }
        },
        {
          "name": "ownerMemberWallet",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114,
                  95,
                  119,
                  97,
                  108,
                  108,
                  101,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "creator"
              }
            ]
          }
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "organizationId",
          "type": "u64"
        },
        {
          "name": "ownerMemberId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "initializeVault",
      "discriminator": [
        48,
        191,
        163,
        44,
        71,
        129,
        63,
        164
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "adminMember"
          ]
        },
        {
          "name": "adminMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "admin_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "vaultId"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "vaultTokenAccount",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "vaultState"
              },
              {
                "kind": "account",
                "path": "tokenProgram"
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        },
        {
          "name": "associatedTokenProgram",
          "address": "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "vaultId",
          "type": "u64"
        }
      ]
    },
    {
      "name": "registerRecipient",
      "discriminator": [
        46,
        231,
        207,
        112,
        215,
        52,
        195,
        125
      ],
      "accounts": [
        {
          "name": "authority",
          "writable": true,
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "registrarMember",
            "vaultState"
          ]
        },
        {
          "name": "registrarMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "registrar_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "recipient",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  99,
                  105,
                  112,
                  105,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "recipientId"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "destinationTokenAccount"
        },
        {
          "name": "systemProgram",
          "address": "11111111111111111111111111111111"
        }
      ],
      "args": [
        {
          "name": "recipientId",
          "type": "u64"
        },
        {
          "name": "destinationWallet",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "rotateRecipientWallet",
      "discriminator": [
        75,
        71,
        69,
        204,
        63,
        75,
        14,
        66
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "registrarMember",
            "vaultState",
            "recipient"
          ]
        },
        {
          "name": "registrarMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "registrar_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "recipient",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  114,
                  101,
                  99,
                  105,
                  112,
                  105,
                  101,
                  110,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "arg",
                "path": "recipientId"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "newDestinationTokenAccount"
        }
      ],
      "args": [
        {
          "name": "recipientId",
          "type": "u64"
        },
        {
          "name": "newDestinationWallet",
          "type": "pubkey"
        }
      ]
    },
    {
      "name": "setOrganizationPaused",
      "discriminator": [
        47,
        133,
        163,
        15,
        69,
        191,
        17,
        232
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "organization",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "adminMember"
          ]
        },
        {
          "name": "adminMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "admin_member.member_id",
                "account": "member"
              }
            ]
          }
        }
      ],
      "args": [
        {
          "name": "paused",
          "type": "bool"
        }
      ]
    },
    {
      "name": "withdrawSplVaultFunds",
      "discriminator": [
        155,
        38,
        110,
        105,
        45,
        201,
        135,
        221
      ],
      "accounts": [
        {
          "name": "authority",
          "signer": true
        },
        {
          "name": "organization",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  111,
                  114,
                  103,
                  97,
                  110,
                  105,
                  122,
                  97,
                  116,
                  105,
                  111,
                  110
                ]
              },
              {
                "kind": "account",
                "path": "organization.creator",
                "account": "organization"
              },
              {
                "kind": "account",
                "path": "organization.organization_id",
                "account": "organization"
              }
            ]
          },
          "relations": [
            "treasuryMember",
            "vaultState"
          ]
        },
        {
          "name": "treasuryMember",
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  109,
                  101,
                  109,
                  98,
                  101,
                  114
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "treasury_member.member_id",
                "account": "member"
              }
            ]
          }
        },
        {
          "name": "vaultState",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "const",
                "value": [
                  118,
                  97,
                  117,
                  108,
                  116
                ]
              },
              {
                "kind": "account",
                "path": "organization"
              },
              {
                "kind": "account",
                "path": "vault_state.vault_id",
                "account": "vaultState"
              }
            ]
          }
        },
        {
          "name": "mint"
        },
        {
          "name": "vaultAta",
          "writable": true,
          "pda": {
            "seeds": [
              {
                "kind": "account",
                "path": "vaultState"
              },
              {
                "kind": "const",
                "value": [
                  6,
                  221,
                  246,
                  225,
                  215,
                  101,
                  161,
                  147,
                  217,
                  203,
                  225,
                  70,
                  206,
                  235,
                  121,
                  172,
                  28,
                  180,
                  133,
                  237,
                  95,
                  91,
                  55,
                  145,
                  58,
                  140,
                  245,
                  133,
                  126,
                  255,
                  0,
                  169
                ]
              },
              {
                "kind": "account",
                "path": "mint"
              }
            ],
            "program": {
              "kind": "const",
              "value": [
                140,
                151,
                37,
                143,
                78,
                36,
                137,
                241,
                187,
                61,
                16,
                41,
                20,
                142,
                13,
                131,
                11,
                90,
                19,
                153,
                218,
                255,
                16,
                132,
                4,
                142,
                123,
                216,
                219,
                233,
                248,
                89
              ]
            }
          }
        },
        {
          "name": "destinationTokenAccount",
          "writable": true
        },
        {
          "name": "tokenProgram",
          "address": "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
        }
      ],
      "args": [
        {
          "name": "amount",
          "type": "u64"
        },
        {
          "name": "destinationWallet",
          "type": "pubkey"
        }
      ]
    }
  ],
  "accounts": [
    {
      "name": "approval",
      "discriminator": [
        233,
        9,
        153,
        49,
        11,
        222,
        59,
        130
      ]
    },
    {
      "name": "approvalPolicyVersion",
      "discriminator": [
        133,
        194,
        77,
        91,
        37,
        113,
        2,
        255
      ]
    },
    {
      "name": "member",
      "discriminator": [
        54,
        19,
        162,
        21,
        29,
        166,
        17,
        198
      ]
    },
    {
      "name": "memberWallet",
      "discriminator": [
        225,
        26,
        52,
        143,
        86,
        46,
        6,
        54
      ]
    },
    {
      "name": "organization",
      "discriminator": [
        145,
        38,
        152,
        251,
        91,
        57,
        118,
        160
      ]
    },
    {
      "name": "payment",
      "discriminator": [
        227,
        231,
        51,
        26,
        244,
        88,
        4,
        148
      ]
    },
    {
      "name": "recipient",
      "discriminator": [
        80,
        186,
        47,
        196,
        232,
        251,
        21,
        148
      ]
    },
    {
      "name": "vaultState",
      "discriminator": [
        228,
        196,
        82,
        165,
        98,
        210,
        235,
        152
      ]
    }
  ],
  "events": [
    {
      "name": "memberCreated",
      "discriminator": [
        119,
        191,
        116,
        88,
        44,
        173,
        169,
        211
      ]
    },
    {
      "name": "organizationInitialized",
      "discriminator": [
        30,
        255,
        39,
        42,
        77,
        225,
        103,
        125
      ]
    },
    {
      "name": "organizationPauseStateChanged",
      "discriminator": [
        120,
        143,
        239,
        97,
        112,
        24,
        203,
        78
      ]
    },
    {
      "name": "paymentApprovalRecorded",
      "discriminator": [
        188,
        199,
        68,
        23,
        124,
        99,
        60,
        166
      ]
    },
    {
      "name": "paymentApprovalThresholdReached",
      "discriminator": [
        208,
        253,
        81,
        24,
        88,
        235,
        186,
        145
      ]
    },
    {
      "name": "paymentCancelled",
      "discriminator": [
        137,
        140,
        226,
        59,
        55,
        152,
        253,
        179
      ]
    },
    {
      "name": "policyVersionCreated",
      "discriminator": [
        32,
        245,
        162,
        150,
        239,
        0,
        42,
        124
      ]
    },
    {
      "name": "recipientWalletRotated",
      "discriminator": [
        77,
        10,
        127,
        86,
        237,
        254,
        122,
        239
      ]
    },
    {
      "name": "splPaymentExecuted",
      "discriminator": [
        225,
        239,
        175,
        239,
        5,
        230,
        7,
        10
      ]
    },
    {
      "name": "vaultFundsWithdrawn",
      "discriminator": [
        246,
        142,
        195,
        55,
        184,
        63,
        143,
        118
      ]
    }
  ],
  "errors": [
    {
      "code": 6000,
      "name": "memberOrganizationMismatch",
      "msg": "The administrative member does not belong to this organization"
    },
    {
      "code": 6001,
      "name": "inactiveMember",
      "msg": "The administrative member is inactive"
    },
    {
      "code": 6002,
      "name": "unauthorizedWallet",
      "msg": "The signer is not the member's authorized wallet"
    },
    {
      "code": 6003,
      "name": "missingAdminRole",
      "msg": "The member does not have the administrator role"
    },
    {
      "code": 6004,
      "name": "invalidMemberRoles",
      "msg": "The supplied member roles are invalid"
    },
    {
      "code": 6005,
      "name": "invalidAuthorizedWallet",
      "msg": "The authorized wallet cannot be the default public key"
    },
    {
      "code": 6006,
      "name": "invalidPolicyId",
      "msg": "Policy ID must be greater than zero"
    },
    {
      "code": 6007,
      "name": "invalidPolicyVersion",
      "msg": "Policy version must be greater than zero"
    },
    {
      "code": 6008,
      "name": "emptyPolicyMembers",
      "msg": "The policy must contain at least one eligible member"
    },
    {
      "code": 6009,
      "name": "tooManyPolicyMembers",
      "msg": "The policy contains too many eligible members"
    },
    {
      "code": 6010,
      "name": "invalidPolicyThreshold",
      "msg": "The approval threshold is invalid"
    },
    {
      "code": 6011,
      "name": "duplicatePolicyMember",
      "msg": "The policy contains the same member more than once"
    },
    {
      "code": 6012,
      "name": "invalidPolicyMember",
      "msg": "An eligible member account is invalid"
    },
    {
      "code": 6013,
      "name": "inactivePolicyMember",
      "msg": "An eligible policy member is inactive"
    },
    {
      "code": 6014,
      "name": "policyMemberMissingApproverRole",
      "msg": "An eligible policy member does not have the approver role"
    },
    {
      "code": 6015,
      "name": "invalidVaultId",
      "msg": "Vault id must be greater than zero"
    },
    {
      "code": 6016,
      "name": "organizationPaused",
      "msg": "The organization is paused"
    },
    {
      "code": 6017,
      "name": "missingRecipientRegistrarRole",
      "msg": "The member must be an admin or preparer"
    },
    {
      "code": 6018,
      "name": "invalidRecipientId",
      "msg": "Recipient id must be greater than zero"
    },
    {
      "code": 6019,
      "name": "invalidRecipientDestination",
      "msg": "Recipient destination cannot be the default public key"
    },
    {
      "code": 6020,
      "name": "vaultOrganizationMismatch",
      "msg": "The vault does not belong to this organization"
    },
    {
      "code": 6021,
      "name": "inactiveVault",
      "msg": "The vault is inactive"
    },
    {
      "code": 6022,
      "name": "unsupportedVaultAsset",
      "msg": "The vault does not support SPL token recipients"
    },
    {
      "code": 6023,
      "name": "vaultMintMismatch",
      "msg": "The supplied mint does not match the vault mint"
    },
    {
      "code": 6024,
      "name": "recipientTokenMintMismatch",
      "msg": "The destination token account has the wrong mint"
    },
    {
      "code": 6025,
      "name": "recipientTokenOwnerMismatch",
      "msg": "The destination token account has the wrong authority"
    },
    {
      "code": 6026,
      "name": "invalidRecipientTokenAccount",
      "msg": "The supplied token account is not the canonical destination ATA"
    },
    {
      "code": 6027,
      "name": "invalidPaymentId",
      "msg": "Payment id must be greater than zero"
    },
    {
      "code": 6028,
      "name": "invalidPaymentAmount",
      "msg": "Payment amount must be greater than zero"
    },
    {
      "code": 6029,
      "name": "invalidExecuteAfter",
      "msg": "Execute-after timestamp cannot be negative"
    },
    {
      "code": 6030,
      "name": "missingPaymentCreatorRole",
      "msg": "The member must be an administrator or preparer"
    },
    {
      "code": 6031,
      "name": "recipientOrganizationMismatch",
      "msg": "The recipient does not belong to this organization"
    },
    {
      "code": 6032,
      "name": "inactiveRecipient",
      "msg": "The recipient is inactive"
    },
    {
      "code": 6033,
      "name": "policyOrganizationMismatch",
      "msg": "The policy does not belong to this organization"
    },
    {
      "code": 6034,
      "name": "inactivePolicy",
      "msg": "The policy version is inactive"
    },
    {
      "code": 6035,
      "name": "missingPaymentApproverRole",
      "msg": "The member must be an administrator or approver"
    },
    {
      "code": 6036,
      "name": "missingPaymentExecutorRole",
      "msg": "The member does not have permission to execute payments"
    },
    {
      "code": 6037,
      "name": "missingPaymentFinalizerRole",
      "msg": "The member does not have permission to finalize payment approval"
    },
    {
      "code": 6038,
      "name": "insufficientVaultBalance",
      "msg": "The vault doesn't have enough balance to cover the payment"
    },
    {
      "code": 6039,
      "name": "paymentTermsMismatch",
      "msg": "The payment terms don't match"
    },
    {
      "code": 6040,
      "name": "paymentNotAcceptingApprovals",
      "msg": "The payment is not accepting approvals"
    },
    {
      "code": 6041,
      "name": "invalidPaymentReservationState",
      "msg": "The payment has an invalid reservation state for approval"
    },
    {
      "code": 6042,
      "name": "approverNotEligibleForPolicy",
      "msg": "The member is not an eligible approver for this policy version"
    },
    {
      "code": 6043,
      "name": "unsupportedSettlementRail",
      "msg": "The payment settlement rail is not supported"
    },
    {
      "code": 6044,
      "name": "invalidRemainingApprovalAccounts",
      "msg": "Remaining approval accounts must be Approval and Member pairs"
    },
    {
      "code": 6045,
      "name": "tooManyApprovalAccounts",
      "msg": "Too many approval witness accounts were supplied"
    },
    {
      "code": 6046,
      "name": "invalidApprovalAccount",
      "msg": "The supplied Approval account is invalid"
    },
    {
      "code": 6047,
      "name": "invalidMemberAccount",
      "msg": "The supplied Member account is invalid"
    },
    {
      "code": 6048,
      "name": "duplicateApprovalMember",
      "msg": "The same member cannot be counted more than once"
    },
    {
      "code": 6049,
      "name": "approvalThresholdNotMet",
      "msg": "The valid approval count has not reached the policy threshold"
    },
    {
      "code": 6050,
      "name": "vaultReservationInvariantViolation",
      "msg": "The vault reserved total exceeds its token balance"
    },
    {
      "code": 6051,
      "name": "arithmeticOverflow",
      "msg": "Arithmetic overflow"
    },
    {
      "code": 6052,
      "name": "invalidPaymentApprovalState",
      "msg": "The payment has an invalid approval state for execution"
    },
    {
      "code": 6053,
      "name": "invalidPaymentReservationExecutionState",
      "msg": "The payment has an invalid reservation state for execution"
    },
    {
      "code": 6054,
      "name": "executionTimeNotReached",
      "msg": "Need to hit the execution time to make the payment"
    },
    {
      "code": 6055,
      "name": "vaultReservationInsufficient",
      "msg": "The vault reserved total is insufficient to make the payment"
    },
    {
      "code": 6056,
      "name": "invalidPaymentCancellationState",
      "msg": "The payment cannot be cancelled from its current payment and reservation states"
    },
    {
      "code": 6057,
      "name": "recipientDestinationUnchanged",
      "msg": "The new recipient destination is the same as the current destination"
    },
    {
      "code": 6058,
      "name": "organizationPauseStateUnchanged",
      "msg": "The organization is already in the requested pause state"
    },
    {
      "code": 6059,
      "name": "missingVaultWithdrawalRole",
      "msg": "The member does not have permission to withdraw vault funds"
    },
    {
      "code": 6060,
      "name": "invalidVaultWithdrawalAmount",
      "msg": "Vault withdrawal amount must be greater than zero"
    },
    {
      "code": 6061,
      "name": "invalidVaultWithdrawalDestination",
      "msg": "The vault withdrawal destination is invalid"
    },
    {
      "code": 6062,
      "name": "vaultWithdrawalExceedsAvailableBalance",
      "msg": "The withdrawal amount exceeds the vault's unreserved balance"
    }
  ],
  "types": [
    {
      "name": "approval",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "payment",
            "type": "pubkey"
          },
          {
            "name": "paymentRevision",
            "type": "u32"
          },
          {
            "name": "policy",
            "type": "pubkey"
          },
          {
            "name": "member",
            "type": "pubkey"
          },
          {
            "name": "memberAuthorizationRevision",
            "type": "u64"
          },
          {
            "name": "authorizationWallet",
            "type": "pubkey"
          },
          {
            "name": "approvedAt",
            "type": "i64"
          },
          {
            "name": "termsHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "approvalPolicyVersion",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "policyId",
            "type": "u64"
          },
          {
            "name": "version",
            "type": "u64"
          },
          {
            "name": "createdByMember",
            "type": "pubkey"
          },
          {
            "name": "eligibleMembers",
            "type": {
              "vec": "pubkey"
            }
          },
          {
            "name": "threshold",
            "type": "u8"
          },
          {
            "name": "enabled",
            "type": "bool"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "asset",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "nativeSol"
          },
          {
            "name": "spl",
            "fields": [
              {
                "name": "mint",
                "type": "pubkey"
              }
            ]
          }
        ]
      }
    },
    {
      "name": "member",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "memberId",
            "type": "u64"
          },
          {
            "name": "authorizedWallet",
            "type": "pubkey"
          },
          {
            "name": "authorizationRevision",
            "type": "u64"
          },
          {
            "name": "roles",
            "type": "u16"
          },
          {
            "name": "active",
            "type": "bool"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "memberCreated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "member",
            "type": "pubkey"
          },
          {
            "name": "memberWallet",
            "type": "pubkey"
          },
          {
            "name": "memberId",
            "type": "u64"
          },
          {
            "name": "authorizedWallet",
            "type": "pubkey"
          },
          {
            "name": "roles",
            "type": "u16"
          },
          {
            "name": "createdByMember",
            "type": "pubkey"
          },
          {
            "name": "createdByWallet",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "memberWallet",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "member",
            "type": "pubkey"
          },
          {
            "name": "authorizedWallet",
            "type": "pubkey"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "organization",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organizationId",
            "type": "u64"
          },
          {
            "name": "creator",
            "type": "pubkey"
          },
          {
            "name": "ownerMember",
            "type": "pubkey"
          },
          {
            "name": "paused",
            "type": "bool"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "organizationInitialized",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "organizationId",
            "type": "u64"
          },
          {
            "name": "creator",
            "type": "pubkey"
          },
          {
            "name": "ownerMember",
            "type": "pubkey"
          },
          {
            "name": "ownerMemberId",
            "type": "u64"
          }
        ]
      }
    },
    {
      "name": "organizationPauseStateChanged",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "previousPaused",
            "type": "bool"
          },
          {
            "name": "newPaused",
            "type": "bool"
          },
          {
            "name": "changedByMember",
            "type": "pubkey"
          },
          {
            "name": "changedByWallet",
            "type": "pubkey"
          },
          {
            "name": "changedAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "payment",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "paymentId",
            "type": "u64"
          },
          {
            "name": "createdBy",
            "type": "pubkey"
          },
          {
            "name": "recipient",
            "type": "pubkey"
          },
          {
            "name": "destination",
            "type": "pubkey"
          },
          {
            "name": "recipientWalletRevision",
            "type": "u32"
          },
          {
            "name": "vault",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "policyVersion",
            "type": "pubkey"
          },
          {
            "name": "paymentRevision",
            "type": "u32"
          },
          {
            "name": "settlementRail",
            "type": {
              "defined": {
                "name": "settlementRail"
              }
            }
          },
          {
            "name": "executeAfter",
            "type": "i64"
          },
          {
            "name": "termsHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "paymentState",
            "type": {
              "defined": {
                "name": "paymentState"
              }
            }
          },
          {
            "name": "reservationState",
            "type": {
              "defined": {
                "name": "reservationState"
              }
            }
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "paymentApprovalRecorded",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "payment",
            "type": "pubkey"
          },
          {
            "name": "approval",
            "type": "pubkey"
          },
          {
            "name": "approverMember",
            "type": "pubkey"
          },
          {
            "name": "authorizationWallet",
            "type": "pubkey"
          },
          {
            "name": "paymentRevision",
            "type": "u32"
          },
          {
            "name": "memberAuthorizationRevision",
            "type": "u64"
          },
          {
            "name": "policyVersion",
            "type": "pubkey"
          },
          {
            "name": "termsHash",
            "type": {
              "array": [
                "u8",
                32
              ]
            }
          },
          {
            "name": "approvedAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "paymentApprovalThresholdReached",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "payment",
            "type": "pubkey"
          },
          {
            "name": "policyVersion",
            "type": "pubkey"
          },
          {
            "name": "vault",
            "type": "pubkey"
          },
          {
            "name": "triggeredByMember",
            "type": "pubkey"
          },
          {
            "name": "validApprovalCount",
            "type": "u8"
          },
          {
            "name": "threshold",
            "type": "u8"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "fundsReserved",
            "type": "bool"
          },
          {
            "name": "vaultReservedTotal",
            "type": "u64"
          },
          {
            "name": "paymentState",
            "type": {
              "defined": {
                "name": "paymentState"
              }
            }
          },
          {
            "name": "processedAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "paymentCancelled",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "payment",
            "type": "pubkey"
          },
          {
            "name": "paymentId",
            "type": "u64"
          },
          {
            "name": "paymentRevision",
            "type": "u32"
          },
          {
            "name": "vault",
            "type": "pubkey"
          },
          {
            "name": "cancelledByMember",
            "type": "pubkey"
          },
          {
            "name": "cancelledByWallet",
            "type": "pubkey"
          },
          {
            "name": "previousPaymentState",
            "type": {
              "defined": {
                "name": "paymentState"
              }
            }
          },
          {
            "name": "previousReservationState",
            "type": {
              "defined": {
                "name": "reservationState"
              }
            }
          },
          {
            "name": "finalReservationState",
            "type": {
              "defined": {
                "name": "reservationState"
              }
            }
          },
          {
            "name": "releasedAmount",
            "type": "u64"
          },
          {
            "name": "remainingReservedTotal",
            "type": "u64"
          },
          {
            "name": "cancelledAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "paymentState",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "pendingApproval"
          },
          {
            "name": "awaitingFunds"
          },
          {
            "name": "approved"
          },
          {
            "name": "paid"
          },
          {
            "name": "cancelled"
          }
        ]
      }
    },
    {
      "name": "policyVersionCreated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "policyVersion",
            "type": "pubkey"
          },
          {
            "name": "policyId",
            "type": "u64"
          },
          {
            "name": "version",
            "type": "u64"
          },
          {
            "name": "threshold",
            "type": "u8"
          },
          {
            "name": "eligibleMemberCount",
            "type": "u8"
          },
          {
            "name": "createdByMember",
            "type": "pubkey"
          },
          {
            "name": "createdByWallet",
            "type": "pubkey"
          }
        ]
      }
    },
    {
      "name": "recipient",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "recipientId",
            "type": "u64"
          },
          {
            "name": "currentDestination",
            "type": "pubkey"
          },
          {
            "name": "walletRevision",
            "type": "u32"
          },
          {
            "name": "active",
            "type": "bool"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    },
    {
      "name": "recipientWalletRotated",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "recipient",
            "type": "pubkey"
          },
          {
            "name": "recipientId",
            "type": "u64"
          },
          {
            "name": "previousDestination",
            "type": "pubkey"
          },
          {
            "name": "newDestination",
            "type": "pubkey"
          },
          {
            "name": "previousWalletRevision",
            "type": "u32"
          },
          {
            "name": "newWalletRevision",
            "type": "u32"
          },
          {
            "name": "rotatedByMember",
            "type": "pubkey"
          },
          {
            "name": "rotatedByWallet",
            "type": "pubkey"
          },
          {
            "name": "rotatedAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "reservationState",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "none"
          },
          {
            "name": "held"
          },
          {
            "name": "consumed"
          },
          {
            "name": "released"
          }
        ]
      }
    },
    {
      "name": "settlementRail",
      "type": {
        "kind": "enum",
        "variants": [
          {
            "name": "publicSol"
          },
          {
            "name": "publicSpl"
          },
          {
            "name": "privateMagicBlock"
          }
        ]
      }
    },
    {
      "name": "splPaymentExecuted",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "payment",
            "type": "pubkey"
          },
          {
            "name": "paymentId",
            "type": "u64"
          },
          {
            "name": "paymentRevision",
            "type": "u32"
          },
          {
            "name": "vault",
            "type": "pubkey"
          },
          {
            "name": "recipient",
            "type": "pubkey"
          },
          {
            "name": "destinationWallet",
            "type": "pubkey"
          },
          {
            "name": "destinationTokenAccount",
            "type": "pubkey"
          },
          {
            "name": "executorMember",
            "type": "pubkey"
          },
          {
            "name": "executorWallet",
            "type": "pubkey"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "remainingReservedTotal",
            "type": "u64"
          },
          {
            "name": "executedAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "vaultFundsWithdrawn",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "vault",
            "type": "pubkey"
          },
          {
            "name": "vaultId",
            "type": "u64"
          },
          {
            "name": "mint",
            "type": "pubkey"
          },
          {
            "name": "destinationWallet",
            "type": "pubkey"
          },
          {
            "name": "destinationTokenAccount",
            "type": "pubkey"
          },
          {
            "name": "withdrawnByMember",
            "type": "pubkey"
          },
          {
            "name": "withdrawnByWallet",
            "type": "pubkey"
          },
          {
            "name": "amount",
            "type": "u64"
          },
          {
            "name": "vaultBalanceBefore",
            "type": "u64"
          },
          {
            "name": "vaultBalanceAfter",
            "type": "u64"
          },
          {
            "name": "reservedTotal",
            "type": "u64"
          },
          {
            "name": "withdrawnAt",
            "type": "i64"
          }
        ]
      }
    },
    {
      "name": "vaultState",
      "type": {
        "kind": "struct",
        "fields": [
          {
            "name": "organization",
            "type": "pubkey"
          },
          {
            "name": "vaultId",
            "type": "u64"
          },
          {
            "name": "asset",
            "type": {
              "defined": {
                "name": "asset"
              }
            }
          },
          {
            "name": "reservedTotal",
            "type": "u64"
          },
          {
            "name": "active",
            "type": "bool"
          },
          {
            "name": "bump",
            "type": "u8"
          }
        ]
      }
    }
  ],
  "constants": [
    {
      "name": "organizationSeed",
      "type": "bytes",
      "value": "[111, 114, 103, 97, 110, 105, 122, 97, 116, 105, 111, 110]"
    }
  ]
};
