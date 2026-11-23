const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("ISAAgentLeaseRegistry - Audit Anchoring", function () {
  let registry;
  let owner;
  let user;

  beforeEach(async function () {
    [owner, user] = await ethers.getSigners();
    const ISAAgentLeaseRegistry = await ethers.getContractFactory("ISAAgentLeaseRegistry");
    registry = await ISAAgentLeaseRegistry.deploy();
    await registry.deployed();
  });

  it("should anchor an audit batch and verify Merkle inclusion proof", async function () {
    const batchId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("batch-test-001"));

    // Two leaves: L1 and L2
    const leaf1 = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("tx-event-1"));
    const leaf2 = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("tx-event-2"));

    // Sorted pair hashing matching verifyAuditProof:
    let root;
    if (leaf1 <= leaf2) {
      root = ethers.utils.keccak256(ethers.utils.concat([leaf1, leaf2]));
    } else {
      root = ethers.utils.keccak256(ethers.utils.concat([leaf2, leaf1]));
    }

    const tx = await registry.anchorAuditBatch(batchId, root, 2, "gcp-production-us-central1");
    await expect(tx)
      .to.emit(registry, "AuditBatchAnchored");

    expect(await registry.getAuditBatchCount()).to.equal(1);

    // Verify proof for leaf1 (sibling is leaf2)
    const proof1 = [leaf2];
    const isValid1 = await registry.verifyAuditProof(batchId, leaf1, proof1);
    expect(isValid1).to.be.true;

    // Verify proof for leaf2 (sibling is leaf1)
    const proof2 = [leaf1];
    const isValid2 = await registry.verifyAuditProof(batchId, leaf2, proof2);
    expect(isValid2).to.be.true;

    // Reject invalid proof
    const fakeLeaf = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("fake-event"));
    const isFakeValid = await registry.verifyAuditProof(batchId, fakeLeaf, proof1);
    expect(isFakeValid).to.be.false;
  });

  it("rejects non-owner audit batch anchoring", async function () {
    const batchId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("batch-test-002"));
    const root = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("root-002"));

    await expect(
      registry.connect(user).anchorAuditBatch(batchId, root, 1, "test-cluster")
    ).to.be.revertedWith("Caller is not the contract owner");
  });

  it("anchors robotic physical execution receipts and verifies inclusion proof (#93)", async function () {
    const batchId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("robot-batch-orca-01"));

    // Telemetry execution receipts from isa_robot:
    const receipt1 = JSON.stringify({
      operation_id: "op-robot-move-1",
      device_id: "orca-hand-eth-01",
      action: "move_joint:index_mcp_flexion",
      status: "completed",
      duration_ms: 152.4,
      lease_id: "0x7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
      completion_verified: true,
    });
    const receipt2 = JSON.stringify({
      operation_id: "op-robot-grasp-2",
      device_id: "orca-hand-eth-01",
      action: "grasp:power",
      status: "completed",
      duration_ms: 320.1,
      lease_id: "0x7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
      completion_verified: true,
    });

    const leaf1 = ethers.utils.keccak256(ethers.utils.toUtf8Bytes(receipt1));
    const leaf2 = ethers.utils.keccak256(ethers.utils.toUtf8Bytes(receipt2));

    let root;
    if (leaf1 <= leaf2) {
      root = ethers.utils.keccak256(ethers.utils.concat([leaf1, leaf2]));
    } else {
      root = ethers.utils.keccak256(ethers.utils.concat([leaf2, leaf1]));
    }

    const tx = await registry.anchorAuditBatch(batchId, root, 2, "sn-robotics-cluster-01");
    await expect(tx).to.emit(registry, "AuditBatchAnchored");

    // Verify proof for robotic receipt1
    const proof1 = [leaf2];
    const isVerified = await registry.verifyAuditProof(batchId, leaf1, proof1);
    expect(isVerified).to.be.true;

    // Verify proof for robotic receipt2
    const proof2 = [leaf1];
    const isVerified2 = await registry.verifyAuditProof(batchId, leaf2, proof2);
    expect(isVerified2).to.be.true;
  });
});
