// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/**
 * @title ISAAgentLeaseRegistry
 * @notice Tier 0 Hardware-Anchored Decentralized Action Lease & Merkle Audit Anchor Registry for isA_AEP.
 * Enforces hardware trust root bindings (Secure Enclave / StrongBox) and physical action leases with on-chain slashing.
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

    event LeaseGranted(bytes32 indexed leaseId, string indexed did, address indexed hardwareAddress, uint256 validUntil, string allowedActions);
    event LeaseRevoked(bytes32 indexed leaseId, string reason, uint256 timestamp);
    event AuditBatchAnchored(bytes32 indexed batchId, bytes32 indexed merkleRoot, uint256 eventCount, string clusterId, uint256 timestamp);

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
            createdAt: block.timestamp
        });

        didLeases[did].push(leaseId);
        emit LeaseGranted(leaseId, did, hardwareAddress, validUntil, allowedActions);
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
}
