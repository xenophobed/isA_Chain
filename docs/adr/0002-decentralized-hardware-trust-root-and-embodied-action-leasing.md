# ADR-0002: Decentralized Hardware Trust Root & Embodied Action Leasing

**Date**: 2026-11-23
**Status**: Accepted

## Context

As the isA platform expands from pure cloud agent swarms to embodied robotics (`isa_robot`) and ambient edge hardware (`isA_Frame`), physical actuation introduces real-world safety, liability, and authorization risks. A compromised or malfunctioning cloud agent cannot be permitted to freely actuate physical robots (such as the 17-DOF ETH Zurich ORCA Hand or RP Hand) without explicit user authorization.

Prior to this decision:
- `isa_robot` exposed unauthenticated FastMCP tool endpoints (`move_joint`, `grasp`, `execute_skill`).
- `isa_mobile` managed hardware Secure Enclave / Android StrongBox DID keys (`did:isa:0x...`) but had no cryptographic tie-in to robot tool calls.
- `isA_Frame` used a standalone companion client (`emoframe-mobile`) with legacy JWT tokens, isolated from the platform's DID identity network.
- `isA_Chain` provided smart contract infrastructure (`ISAAgentLeaseRegistry.sol`), but lacked integration with edge telemetry streams.

## Decision

Adopt a **Hybrid Enclave Voucher Architecture** with asynchronous Merkle audit anchoring on `isA_Chain`:

1. **Hardware DID as Root of Trust**: Mobile devices (`isa_mobile`) utilize native Secure Enclave / Keymaster biometric signing (FaceID / TouchID) to issue time-bound, scoped cryptographic action vouchers (`HardwareActionVoucher`).
2. **Local RAL Pre-execution Enforcement**: `isa_robot` implements `ral_auth_guard` in `src/isa_robot/mcp/tools.py`. FastMCP tools validate voucher expiration (`valid_until`), target device binding (`target_device_id`), and capability scoping (`allowed_capabilities`) in-memory (< 5ms) before servo torque is applied.
3. **Asynchronous Merkle Telemetry Anchoring**: Physical execution receipts (`operation_id`, `device_id`, `action`, `lease_id`, `duration_ms`, `status`) are streamed over NATS (`robot.telemetry.executed`). These are batched into 10-minute Merkle trees and anchored into `ISAAgentLeaseRegistry.sol` via `anchorAuditBatch`, enabling on-chain proof of inclusion via `verifyAuditProof`.
4. **Unified Ambient Display Onboarding**: `isA_Frame` device onboarding is unified into `isa_mobile` using camera QR scanning (`EMOFRAME:<deviceId>:<token>`), binding display devices directly to the owner's `did:isa` decentralized identity and deprecating the standalone `emoframe-mobile` client.

## Alternatives Considered

1. **Direct Synchronous On-Chain Query per Tool Call** — Rejected. Querying an RPC node for every joint command introduces 200ms–1500ms latency, stalling real-time motor control loops, and causes system failure during local network disconnects.
2. **Centralized Cloud Gateway / Proxy Validation** — Rejected. Routing all physical commands through a cloud BFF re-introduces single points of failure and cloud egress latency, defeating the purpose of decentralized trust and local safety boundaries.

## Consequences

### Positive
- **Physical Safety Boundary**: Servos and actuators will not engage without a cryptographically valid, biometrically signed lease voucher.
- **Sub-5ms Execution Latency**: Offline cryptographic verification guarantees immediate joint trajectory response without blocking on blockchain blocks.
- **Unified Identity & UX**: Users manage IoT displays, smart picture frames, and robotic hands within a single `isa_mobile` application.
- **Immutable Auditability**: Physical action receipts are Merkle-anchored to `isA_Chain`, providing mathematically verifiable compliance proofs.

### Negative
- **Voucher Forwarding Overhead**: Agents orchestrating physical robots must request and attach the `lease_voucher` payload in tool invocation arguments.
- **Clock Drift Sensitivity**: Devices must maintain synchronized time (via NTP) to correctly evaluate `valid_until` lease expiration timestamps.

### Risks
- **Network Partitioning During Mid-Action Revocation**: If a lease is revoked on-chain while the robot is disconnected from the internet, the local robot relies on lease TTL.
  - *Mitigation*: Hardware action vouchers are strictly short-lived (default 60–300 seconds), ensuring fail-closed protection.

## References
- `xenoISA/isa_robot#30` (Epic: hardware — Enforce action leases and audit physical actuation)
- `xenoISA/isa_robot#31` (PR #33: Enforce HardwareActionVoucher in FastMCP tools)
- `xenoISA/isa_robot#32` (PR #34: Publish robotic physical execution receipts over NATS)
- `xenoISA/isa_mobile#83` (Epic: hardware — Embodied robotics action leasing and ambient frame pairing)
- `xenoISA/isa_mobile#84` (PR #86: Generate verifiable HardwareActionVoucher with Secure Enclave DID)
- `xenoISA/isa_mobile#85` (PR #87: Unify EmoFrame ambient display QR pairing into mobile app)
- `xenophobed/isA_Chain#93` (PR #94: Verify and anchor hardware robotic execution receipts to ISAAgentLeaseRegistry)
- `xenoISA/isA_Frame#70` (PR #70: Deprecate emoframe-mobile in favor of unified isa_mobile)
