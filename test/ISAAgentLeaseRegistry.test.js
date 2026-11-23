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
});
