const { expect } = require("chai");
const { ethers } = require("hardhat");
const {
  hashReceipt,
  combineSorted,
  buildMerkleTree,
  generateProof,
  TelemetryAuditBatcher,
} = require("../services/oracle-relayer/telemetry_anchor_relayer");

describe("TelemetryAuditRelayer & On-Chain Audit Anchoring", function () {
  let registry;
  let owner;

  beforeEach(async function () {
    [owner] = await ethers.getSigners();
    const ISAAgentLeaseRegistry = await ethers.getContractFactory("ISAAgentLeaseRegistry");
    registry = await ISAAgentLeaseRegistry.deploy();
    await registry.deployed();
  });

  describe("Deterministic Hashing & Merkle Helpers", function () {
    it("produces identical hashes regardless of object key order", function () {
      const receiptA = {
        operation_id: "op-1",
        device_id: "orca-01",
        status: "completed",
        duration_ms: 120,
      };
      const receiptB = {
        duration_ms: 120,
        status: "completed",
        device_id: "orca-01",
        operation_id: "op-1",
      };

      const hashA = hashReceipt(receiptA);
      const hashB = hashReceipt(receiptB);
      expect(hashA).to.equal(hashB);
      expect(hashA).to.match(/^0x[a-f0-9]{64}$/);
    });

    it("matches contract sorted pair hashing", function () {
      const leaf1 = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("leaf1"));
      const leaf2 = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("leaf2"));

      const combinedJS = combineSorted(leaf1, leaf2);

      let expectedSolidity;
      if (leaf1.toLowerCase() <= leaf2.toLowerCase()) {
        expectedSolidity = ethers.utils.keccak256(ethers.utils.concat([leaf1, leaf2]));
      } else {
        expectedSolidity = ethers.utils.keccak256(ethers.utils.concat([leaf2, leaf1]));
      }

      expect(combinedJS).to.equal(expectedSolidity);
    });
  });

  describe("TelemetryAuditBatcher Lifecycle", function () {
    it("batches receipts, anchors to ISAAgentLeaseRegistry, and verifies on-chain proofs", async function () {
      const batcher = new TelemetryAuditBatcher({
        clusterId: "sn-robotics-cluster-01",
      });

      const sampleReceipts = [
        {
          operation_id: "op-orca-01",
          device_id: "orca-eth-hand",
          action: "move_joint:index_flexion",
          status: "completed",
          duration_ms: 145.2,
          lease_id: "0x" + "1".repeat(64),
          completion_verified: true,
        },
        {
          operation_id: "op-orca-02",
          device_id: "orca-eth-hand",
          action: "grasp:power",
          status: "completed",
          duration_ms: 280.4,
          lease_id: "0x" + "1".repeat(64),
          completion_verified: true,
        },
        {
          operation_id: "op-orca-03",
          device_id: "orca-eth-hand",
          action: "move_joint:thumb_rotation",
          status: "completed",
          duration_ms: 98.7,
          lease_id: "0x" + "1".repeat(64),
          completion_verified: true,
        },
        {
          operation_id: "op-orca-04",
          device_id: "orca-eth-hand",
          action: "release:open",
          status: "completed",
          duration_ms: 195.0,
          lease_id: "0x" + "1".repeat(64),
          completion_verified: true,
        },
      ];

      // Enqueue all 4 receipts
      sampleReceipts.forEach((r) => batcher.enqueueReceipt(r));

      // Flush and anchor
      const customBatchId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("batch-telemetry-run-001"));
      const result = await batcher.flushAndAnchor(registry, customBatchId);

      expect(result.batchId).to.equal(customBatchId);
      expect(result.eventCount).to.equal(4);
      expect(await registry.getAuditBatchCount()).to.equal(1);

      // Verify each receipt on-chain
      for (const r of sampleReceipts) {
        const proofData = batcher.getProofForOperation(r.operation_id);
        expect(proofData.batchId).to.equal(customBatchId);
        expect(proofData.root).to.equal(result.root);

        const isValid = await registry.verifyAuditProof(
          proofData.batchId,
          proofData.leaf,
          proofData.proof
        );
        expect(isValid).to.be.true;
      }

      // Reject tampered leaf
      const tamperedLeaf = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("tampered-receipt"));
      const proof0 = batcher.getProofForOperation("op-orca-01").proof;
      const isTamperedValid = await registry.verifyAuditProof(customBatchId, tamperedLeaf, proof0);
      expect(isTamperedValid).to.be.false;
    });

    it("correctly handles odd number of receipts (e.g. 3 receipts)", async function () {
      const batcher = new TelemetryAuditBatcher({
        clusterId: "sn-robotics-cluster-01",
      });

      const threeReceipts = [
        { operation_id: "op-odd-1", device_id: "orca-01", action: "grip", status: "completed" },
        { operation_id: "op-odd-2", device_id: "orca-01", action: "lift", status: "completed" },
        { operation_id: "op-odd-3", device_id: "orca-01", action: "drop", status: "completed" },
      ];

      threeReceipts.forEach((r) => batcher.enqueueReceipt(r));

      const batchId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("batch-odd-3"));
      const result = await batcher.flushAndAnchor(registry, batchId);

      expect(result.eventCount).to.equal(3);

      for (const r of threeReceipts) {
        const proofData = batcher.getProofForOperation(r.operation_id);
        const isValid = await registry.verifyAuditProof(batchId, proofData.leaf, proofData.proof);
        expect(isValid).to.be.true;
      }
    });
  });
});
