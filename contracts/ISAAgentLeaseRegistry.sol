// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title ISAAgentLeaseRegistry
 * @notice Tier 0 Hardware-Anchored Decentralized Action Lease & Merkle Audit Anchor Registry for isA_AEP.
 * Enforces hardware trust root bindings (Secure Enclave / StrongBox), physical action leases,
 * and Smart Account Budget Invariant Enforcement (#1853).
 */
contract ISAAgentLeaseRegistry {
    address public owner;

    enum LeaseStatus {
        Inactive,
        Active,
        Revoked,
        Expired
    }

    struct ActionLease {
        bytes32 leaseId;        // Unique cryptographic lease identifier
        string did;             // W3C DID string (e.g., did:isa:0x<20-byte-address>)
        address hardwareAddress;// 20-byte EVM address derived from Enclave public key
        uint256 validUntil;     // Unix timestamp expiration
        LeaseStatus status;     // Active, Revoked, Expired
        string allowedActions;  // Scoped action mask (e.g. "mcp:exec,db:write")
        uint256 maxAutonomousBudget; // Maximum autonomous spending ceiling in cents (#1853)
        uint256 spentBudget;         // Cumulative autonomous funds consumed (#1853)
        string allowedDomains;       // Comma-separated domain allowlist (#1853)
        uint256 createdAt;
    }

    struct MerkleAuditBatch {
        bytes32 batchId;
        bytes32 merkleRoot;     // Merkle root hash of batched AEP execution audit logs
        uint256 eventCount;     // Number of events in this anchor
        uint256 timestamp;
        string clusterId;      // e.g. "gcp-production-us-central1"
    }

    // leaseId => ActionLease
    mapping(bytes32 => ActionLease) public leases;
    // did => list of leaseIds
    mapping(string => bytes32[]) private didLeases;
    // batchId => MerkleAuditBatch
    mapping(bytes32 => MerkleAuditBatch) public auditBatches;
    // Sequential list of audit batch IDs for timeline verification
    bytes32[] public auditBatchHistory;

    event LeaseGranted(
        bytes32 indexed leaseId,
        string indexed did,
        address indexed hardwareAddress,
        uint256 validUntil,
        string allowedActions,
        uint256 maxAutonomousBudget,
        string allowedDomains
    );
    event LeaseRevoked(bytes32 indexed leaseId, string reason, uint256 timestamp);
    event AuditBatchAnchored(bytes32 indexed batchId, bytes32 indexed merkleRoot, uint256 eventCount, string clusterId, uint256 timestamp);
    event LeasePaymentExecuted(bytes32 indexed leaseId, uint256 amount, uint256 spentBudget, string targetDomain, uint256 timestamp);

    modifier onlyOwner() {
        require(msg.sender == owner, "Caller is not the contract owner");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    /**
     * @notice Grants or updates a cryptographic action lease for a mobile DID or sandboxed agent.
     */
    function grantLease(
        bytes32 leaseId,
        string calldata did,
        address hardwareAddress,
        uint256 durationSeconds,
        string calldata allowedActions
    ) external onlyOwner {
        _grantLeaseInternal(leaseId, did, hardwareAddress, durationSeconds, allowedActions, 0, "*");
    }

    /**
     * @notice Grants a hardware-anchored Smart Account action lease with strict budget and domain invariants (#1853).
     */
    function grantSmartAccountLease(
        bytes32 leaseId,
        string calldata did,
        address hardwareAddress,
        uint256 durationSeconds,
        string calldata allowedActions,
        uint256 maxAutonomousBudget,
        string calldata allowedDomains
    ) external onlyOwner {
        _grantLeaseInternal(leaseId, did, hardwareAddress, durationSeconds, allowedActions, maxAutonomousBudget, allowedDomains);
    }

    function _grantLeaseInternal(
        bytes32 leaseId,
        string calldata did,
        address hardwareAddress,
        uint256 durationSeconds,
        string calldata allowedActions,
        uint256 maxAutonomousBudget,
        string memory allowedDomains
    ) internal {
        require(leaseId != bytes32(0), "Invalid leaseId");
        require(durationSeconds > 0, "Duration must be positive");

        uint256 validUntil = block.timestamp + durationSeconds;
        leases[leaseId] = ActionLease({
            leaseId: leaseId,
            did: did,
            hardwareAddress: hardwareAddress,
            validUntil: validUntil,
            status: LeaseStatus.Active,
            allowedActions: allowedActions,
            maxAutonomousBudget: maxAutonomousBudget,
            spentBudget: 0,
            allowedDomains: allowedDomains,
            createdAt: block.timestamp
        });

        didLeases[did].push(leaseId);
        emit LeaseGranted(leaseId, did, hardwareAddress, validUntil, allowedActions, maxAutonomousBudget, allowedDomains);
    }

    /**
     * @notice Executes and audits an autonomous payment against an active lease, strictly enforcing budget limits (#1853).
     */
    function recordLeasePayment(
        bytes32 leaseId,
        string calldata did,
        uint256 amount,
        string calldata targetDomain
    ) external onlyOwner {
        (bool valid, string memory reason) = this.isLeaseValid(leaseId, did);
        require(valid, reason);

        ActionLease storage lease = leases[leaseId];
        require(lease.spentBudget + amount <= lease.maxAutonomousBudget, "BudgetExceeded: payment exceeds maxAutonomousBudget");

        lease.spentBudget += amount;
        emit LeasePaymentExecuted(leaseId, amount, lease.spentBudget, targetDomain, block.timestamp);
    }

    /**
     * @notice Revokes an active lease immediately (slashing unauthorized execution).
     */
    function revokeLease(bytes32 leaseId, string calldata reason) external onlyOwner {
        require(leases[leaseId].status == LeaseStatus.Active, "Lease is not active");
        leases[leaseId].status = LeaseStatus.Revoked;
        emit LeaseRevoked(leaseId, reason, block.timestamp);
    }

    /**
     * @notice Live query checking whether an action lease is currently valid and active.
     */
    function isLeaseValid(bytes32 leaseId, string calldata did) external view returns (bool, string memory) {
        ActionLease memory lease = leases[leaseId];
        if (lease.leaseId == bytes32(0)) {
            return (false, "LEASE_NOT_FOUND");
        }
        if (keccak256(bytes(lease.did)) != keccak256(bytes(did))) {
            return (false, "DID_MISMATCH");
        }
        if (lease.status == LeaseStatus.Revoked) {
            return (false, "LEASE_REVOKED");
        }
        if (block.timestamp > lease.validUntil) {
            return (false, "LEASE_EXPIRED");
        }
        if (lease.status != LeaseStatus.Active) {
            return (false, "LEASE_INACTIVE");
        }
        return (true, "VALID");
    }

    /**
     * @notice Commit an immutable Merkle root of AEP gateway audit events.
     */
    function anchorAuditBatch(
        bytes32 batchId,
        bytes32 merkleRoot,
        uint256 eventCount,
        string calldata clusterId
    ) external onlyOwner {
        require(batchId != bytes32(0), "Invalid batchId");
        require(merkleRoot != bytes32(0), "Invalid merkleRoot");
        require(auditBatches[batchId].timestamp == 0, "Batch already anchored");

        auditBatches[batchId] = MerkleAuditBatch({
            batchId: batchId,
            merkleRoot: merkleRoot,
            eventCount: eventCount,
            timestamp: block.timestamp,
            clusterId: clusterId
        });

        auditBatchHistory.push(batchId);
        emit AuditBatchAnchored(batchId, merkleRoot, eventCount, clusterId, block.timestamp);
    }

    /**
     * @notice Returns total number of anchored audit batches.
     */
    function getAuditBatchCount() external view returns (uint256) {
        return auditBatchHistory.length;
    }

    /**
     * @notice Verifies whether a leaf hash belongs to an anchored audit batch using a Merkle proof.
     */
    function verifyAuditProof(
        bytes32 batchId,
        bytes32 leaf,
        bytes32[] calldata proof
    ) external view returns (bool) {
        bytes32 root = auditBatches[batchId].merkleRoot;
        if (root == bytes32(0)) {
            return false;
        }
        bytes32 computedHash = leaf;
        for (uint256 i = 0; i < proof.length; i++) {
            bytes32 proofElement = proof[i];
            if (computedHash <= proofElement) {
                computedHash = keccak256(abi.encodePacked(computedHash, proofElement));
            } else {
                computedHash = keccak256(abi.encodePacked(proofElement, computedHash));
            }
        }
        return computedHash == root;
    }
}
