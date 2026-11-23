/**
 * Telemetry Audit Anchor Relayer for isA_Chain.
 *
 * Bridges robotic execution receipts (from isa_robot NATS topic robot.telemetry.executed)
 * and batches them into cryptographic Merkle trees anchored to ISAAgentLeaseRegistry.sol.
 */

const { ethers } = require("ethers");

/**
 * Computes deterministic canonical keccak256 hash of a robotic telemetry receipt.
 * @param {object} receipt
 * @returns {string} 0x-prefixed 32-byte hex string
 */
function hashReceipt(receipt) {
  // Sort keys deterministically
  const sortedKeys = Object.keys(receipt).sort();
  const sortedObj = {};
  for (const k of sortedKeys) {
    sortedObj[k] = receipt[k];
  }
  const jsonStr = JSON.stringify(sortedObj);
  return ethers.utils.keccak256(ethers.utils.toUtf8Bytes(jsonStr));
}

/**
 * Hashes two sibling nodes with sorted ordering matching ISAAgentLeaseRegistry.sol:
 * keccak256(abi.encodePacked(min, max))
 * @param {string} a 32-byte hex
 * @param {string} b 32-byte hex
 * @returns {string} 32-byte hex
 */
function combineSorted(a, b) {
  if (a.toLowerCase() <= b.toLowerCase()) {
    return ethers.utils.keccak256(ethers.utils.concat([a, b]));
  } else {
    return ethers.utils.keccak256(ethers.utils.concat([b, a]));
  }
}

/**
 * Builds Merkle tree levels from leaf hashes.
 * @param {string[]} leaves
 * @returns {string[][]} Array of levels, from leaves (level 0) to root (last level)
 */
function buildMerkleTree(leaves) {
  if (!leaves || leaves.length === 0) {
    throw new Error("Cannot build Merkle tree from empty leaves");
  }
  const levels = [[...leaves]];

  while (levels[levels.length - 1].length > 1) {
    const currentLevel = levels[levels.length - 1];
    const nextLevel = [];

    for (let i = 0; i < currentLevel.length; i += 2) {
      if (i + 1 < currentLevel.length) {
        nextLevel.push(combineSorted(currentLevel[i], currentLevel[i + 1]));
      } else {
        // Odd number of nodes: duplicate last node
        nextLevel.push(combineSorted(currentLevel[i], currentLevel[i]));
      }
    }
    levels.push(nextLevel);
  }

  return levels;
}

/**
 * Generates Merkle inclusion proof for a leaf at index in leaves.
 * @param {string[]} leaves
 * @param {number} leafIndex
 * @returns {string[]} Array of sibling hashes
 */
function generateProof(leaves, leafIndex) {
  if (leafIndex < 0 || leafIndex >= leaves.length) {
    throw new Error(`Leaf index ${leafIndex} out of bounds`);
  }

  const levels = buildMerkleTree(leaves);
  const proof = [];
  let index = leafIndex;

  for (let i = 0; i < levels.length - 1; i++) {
    const level = levels[i];
    const isRight = index % 2 === 1;
    let siblingIndex;

    if (isRight) {
      siblingIndex = index - 1;
    } else {
      siblingIndex = index + 1 < level.length ? index + 1 : index;
    }

    proof.push(level[siblingIndex]);
    index = Math.floor(index / 2);
  }

  return proof;
}

/**
 * In-memory Telemetry Audit Batcher & Relayer.
 */
class TelemetryAuditBatcher {
  constructor(options = {}) {
    this.maxBatchSize = options.maxBatchSize || 50;
    this.clusterId = options.clusterId || "gcp-production-us-central1";
    this.buffer = [];
    this.anchoredBatches = new Map(); // batchId => { root, receipts, leaves, timestamp, txHash }
    this.operationIndex = new Map();  // operationId => { batchId, leafIndex, receipt }
  }

  /**
   * Enqueues a telemetry receipt.
   * @param {object} receipt
   */
  enqueueReceipt(receipt) {
    if (!receipt.operation_id) {
      throw new Error("Receipt missing required field 'operation_id'");
    }
    this.buffer.push(receipt);
  }

  /**
   * Flushes current buffer and anchors to ISAAgentLeaseRegistry contract.
   * @param {object} registryContract Ethers contract instance
   * @param {string} [customBatchId] Optional explicit batch ID
   * @returns {Promise<object>} Anchor result
   */
  async flushAndAnchor(registryContract, customBatchId = null) {
    if (this.buffer.length === 0) {
      return null;
    }

    const receipts = [...this.buffer];
    this.buffer = [];

    const leaves = receipts.map((r) => hashReceipt(r));
    const levels = buildMerkleTree(leaves);
    const root = levels[levels.length - 1][0];

    const batchId =
      customBatchId ||
      ethers.utils.keccak256(
        ethers.utils.toUtf8Bytes(`batch-${Date.now()}-${Math.random().toString(36).slice(2)}`)
      );

    const tx = await registryContract.anchorAuditBatch(
      batchId,
      root,
      receipts.length,
      this.clusterId
    );
    const receipt = await tx.wait();

    // Store batch data for proof generation
    this.anchoredBatches.set(batchId, {
      batchId,
      root,
      receipts,
      leaves,
      timestamp: Date.now(),
      txHash: receipt.transactionHash,
    });

    receipts.forEach((r, idx) => {
      this.operationIndex.set(r.operation_id, {
        batchId,
        leafIndex: idx,
        receipt: r,
      });
    });

    return {
      batchId,
      root,
      eventCount: receipts.length,
      txHash: receipt.transactionHash,
    };
  }

  /**
   * Retrieves inclusion proof for an operation ID.
   * @param {string} operationId
   * @returns {object} { batchId, root, leaf, proof, receipt }
   */
  getProofForOperation(operationId) {
    const entry = this.operationIndex.get(operationId);
    if (!entry) {
      throw new Error(`Operation ${operationId} not found in anchored batches`);
    }

    const batch = this.anchoredBatches.get(entry.batchId);
    const leaf = batch.leaves[entry.leafIndex];
    const proof = generateProof(batch.leaves, entry.leafIndex);

    return {
      batchId: entry.batchId,
      root: batch.root,
      leaf,
      proof,
      receipt: entry.receipt,
    };
  }
}

module.exports = {
  hashReceipt,
  combineSorted,
  buildMerkleTree,
  generateProof,
  TelemetryAuditBatcher,
};
