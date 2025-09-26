/**
 * @fileoverview Blockchain Service - Core blockchain interaction service
 * 
 * Provides low-level blockchain operations and network status monitoring
 */

import { ethers } from 'ethers';

export interface BlockchainStatus {
  network: string;
  chainId: number;
  blockNumber: number;
  gasPrice: string;
  networkFee: string;
  isHealthy: boolean;
  timestamp: number;
  validators: number;
  totalSupply: string;
}

export interface TransactionStatus {
  hash: string;
  status: 'pending' | 'confirmed' | 'failed';
  blockNumber?: number;
  gasUsed?: string;
  effectiveGasPrice?: string;
  confirmations: number;
  timestamp?: number;
}

export interface BlockInfo {
  number: number;
  hash: string;
  timestamp: number;
  transactions: string[];
  gasUsed: string;
  gasLimit: string;
  baseFeePerGas?: string;
}

/**
 * Blockchain Service
 * Handles core blockchain operations and monitoring
 */
export class BlockchainService {
  private provider: ethers.JsonRpcProvider;
  private networkInfo: any;

  constructor(provider: ethers.JsonRpcProvider) {
    this.provider = provider;
    this.initializeNetwork();
  }

  /**
   * Initialize network information
   */
  private async initializeNetwork(): Promise<void> {
    try {
      this.networkInfo = await this.provider.getNetwork();
      console.log(`Connected to network: ${this.networkInfo.name} (${this.networkInfo.chainId})`);
    } catch (error) {
      console.error('Failed to initialize network:', error);
      throw error;
    }
  }

  /**
   * Get current blockchain status
   */
  async getStatus(): Promise<BlockchainStatus> {
    try {
      const [blockNumber, gasPrice, network] = await Promise.all([
        this.provider.getBlockNumber(),
        this.provider.getFeeData(),
        this.provider.getNetwork(),
      ]);

      const block = await this.provider.getBlock(blockNumber);
      
      return {
        network: network.name || 'unknown',
        chainId: Number(network.chainId),
        blockNumber,
        gasPrice: ethers.formatUnits(gasPrice.gasPrice || 0n, 'gwei'),
        networkFee: ethers.formatUnits(gasPrice.maxFeePerGas || 0n, 'gwei'),
        isHealthy: true,
        timestamp: block?.timestamp || Date.now() / 1000,
        validators: 21, // Placeholder - would come from consensus layer
        totalSupply: '1000000000', // Placeholder - would come from token contract
      };
    } catch (error) {
      console.error('Error getting blockchain status:', error);
      return {
        network: 'unknown',
        chainId: 0,
        blockNumber: 0,
        gasPrice: '0',
        networkFee: '0',
        isHealthy: false,
        timestamp: Date.now() / 1000,
        validators: 0,
        totalSupply: '0',
      };
    }
  }

  /**
   * Get transaction status
   */
  async getTransactionStatus(txHash: string): Promise<TransactionStatus> {
    try {
      const [tx, receipt] = await Promise.all([
        this.provider.getTransaction(txHash),
        this.provider.getTransactionReceipt(txHash),
      ]);

      if (!tx) {
        throw new Error(`Transaction ${txHash} not found`);
      }

      let status: 'pending' | 'confirmed' | 'failed' = 'pending';
      let gasUsed: string | undefined;
      let effectiveGasPrice: string | undefined;

      if (receipt) {
        status = receipt.status === 1 ? 'confirmed' : 'failed';
        gasUsed = receipt.gasUsed.toString();
        effectiveGasPrice = ethers.formatUnits(receipt.gasPrice, 'gwei');
      }

      const currentBlock = await this.provider.getBlockNumber();
      const confirmations = receipt ? currentBlock - receipt.blockNumber + 1 : 0;

      return {
        hash: txHash,
        status,
        blockNumber: receipt?.blockNumber,
        gasUsed,
        effectiveGasPrice,
        confirmations,
        timestamp: tx.blockNumber ? (await this.provider.getBlock(tx.blockNumber))?.timestamp : undefined,
      };
    } catch (error) {
      console.error('Error getting transaction status:', error);
      throw error;
    }
  }

  /**
   * Get block information
   */
  async getBlock(blockNumber: number | 'latest' = 'latest'): Promise<BlockInfo> {
    try {
      const block = await this.provider.getBlock(blockNumber, false);
      
      if (!block) {
        throw new Error(`Block ${blockNumber} not found`);
      }

      return {
        number: block.number,
        hash: block.hash,
        timestamp: block.timestamp,
        transactions: block.transactions,
        gasUsed: block.gasUsed.toString(),
        gasLimit: block.gasLimit.toString(),
        baseFeePerGas: block.baseFeePerGas?.toString(),
      };
    } catch (error) {
      console.error('Error getting block info:', error);
      throw error;
    }
  }

  /**
   * Estimate gas for a transaction
   */
  async estimateGas(transaction: {
    to: string;
    data?: string;
    value?: string;
    from?: string;
  }): Promise<{
    gasLimit: string;
    gasPrice: string;
    totalCost: string;
  }> {
    try {
      const gasLimit = await this.provider.estimateGas(transaction);
      const feeData = await this.provider.getFeeData();
      const gasPrice = feeData.gasPrice || 0n;
      
      const totalCost = gasLimit * gasPrice;

      return {
        gasLimit: gasLimit.toString(),
        gasPrice: ethers.formatUnits(gasPrice, 'gwei'),
        totalCost: ethers.formatEther(totalCost),
      };
    } catch (error) {
      console.error('Error estimating gas:', error);
      throw error;
    }
  }

  /**
   * Get logs for a specific contract and event
   */
  async getLogs(filter: {
    address?: string;
    topics?: string[];
    fromBlock?: number | 'latest';
    toBlock?: number | 'latest';
  }): Promise<ethers.Log[]> {
    try {
      return await this.provider.getLogs(filter);
    } catch (error) {
      console.error('Error getting logs:', error);
      throw error;
    }
  }

  /**
   * Wait for transaction confirmation
   */
  async waitForTransaction(
    txHash: string, 
    confirmations: number = 1, 
    timeout: number = 300000
  ): Promise<ethers.TransactionReceipt | null> {
    try {
      return await this.provider.waitForTransaction(txHash, confirmations, timeout);
    } catch (error) {
      console.error('Error waiting for transaction:', error);
      throw error;
    }
  }

  /**
   * Get current gas prices with different speed options
   */
  async getGasPrices(): Promise<{
    slow: string;
    standard: string;
    fast: string;
    instant: string;
  }> {
    try {
      const feeData = await this.provider.getFeeData();
      const baseGas = feeData.gasPrice || 0n;

      return {
        slow: ethers.formatUnits(baseGas * 80n / 100n, 'gwei'), // -20%
        standard: ethers.formatUnits(baseGas, 'gwei'),
        fast: ethers.formatUnits(baseGas * 120n / 100n, 'gwei'), // +20%
        instant: ethers.formatUnits(baseGas * 150n / 100n, 'gwei'), // +50%
      };
    } catch (error) {
      console.error('Error getting gas prices:', error);
      throw error;
    }
  }

  /**
   * Get network statistics
   */
  async getNetworkStats(): Promise<{
    totalTransactions: number;
    totalBlocks: number;
    avgBlockTime: number;
    networkHashrate: string;
    difficulty: string;
  }> {
    try {
      const currentBlock = await this.provider.getBlockNumber();
      const recentBlocks = await Promise.all([
        this.provider.getBlock(currentBlock),
        this.provider.getBlock(currentBlock - 1),
        this.provider.getBlock(currentBlock - 2),
        this.provider.getBlock(currentBlock - 3),
        this.provider.getBlock(currentBlock - 4),
      ]);

      const avgBlockTime = recentBlocks.reduce((acc, block, idx) => {
        if (idx === 0) return acc;
        const prevBlock = recentBlocks[idx - 1];
        if (block && prevBlock) {
          return acc + (prevBlock.timestamp - block.timestamp);
        }
        return acc;
      }, 0) / (recentBlocks.length - 1);

      return {
        totalTransactions: currentBlock * 100, // Approximation
        totalBlocks: currentBlock,
        avgBlockTime,
        networkHashrate: '1000000', // Placeholder
        difficulty: '1000000', // Placeholder
      };
    } catch (error) {
      console.error('Error getting network stats:', error);
      throw error;
    }
  }

  /**
   * Check if an address is a contract
   */
  async isContract(address: string): Promise<boolean> {
    try {
      const code = await this.provider.getCode(address);
      return code !== '0x';
    } catch (error) {
      console.error('Error checking if address is contract:', error);
      return false;
    }
  }

  /**
   * Get balance of native token
   */
  async getNativeBalance(address: string): Promise<string> {
    try {
      const balance = await this.provider.getBalance(address);
      return ethers.formatEther(balance);
    } catch (error) {
      console.error('Error getting native balance:', error);
      throw error;
    }
  }

  /**
   * Get nonce for an address
   */
  async getNonce(address: string): Promise<number> {
    try {
      return await this.provider.getTransactionCount(address);
    } catch (error) {
      console.error('Error getting nonce:', error);
      throw error;
    }
  }

  /**
   * Broadcast a signed transaction
   */
  async broadcastTransaction(signedTx: string): Promise<string> {
    try {
      const response = await this.provider.broadcastTransaction(signedTx);
      return response.hash;
    } catch (error) {
      console.error('Error broadcasting transaction:', error);
      throw error;
    }
  }

  /**
   * Get provider instance
   */
  getProvider(): ethers.JsonRpcProvider {
    return this.provider;
  }

  /**
   * Get network information
   */
  getNetwork(): any {
    return this.networkInfo;
  }

  /**
   * Monitor network health
   */
  async monitorHealth(): Promise<boolean> {
    try {
      const startTime = Date.now();
      await this.provider.getBlockNumber();
      const responseTime = Date.now() - startTime;
      
      // Consider network healthy if response time is less than 5 seconds
      return responseTime < 5000;
    } catch (error) {
      console.error('Network health check failed:', error);
      return false;
    }
  }

  /**
   * Get EIP-1559 gas fee recommendations
   */
  async getEIP1559Fees(): Promise<{
    slow: { maxFeePerGas: string; maxPriorityFeePerGas: string };
    standard: { maxFeePerGas: string; maxPriorityFeePerGas: string };
    fast: { maxFeePerGas: string; maxPriorityFeePerGas: string };
  }> {
    try {
      const feeData = await this.provider.getFeeData();
      const baseFee = feeData.maxFeePerGas || 0n;
      const priorityFee = feeData.maxPriorityFeePerGas || 0n;

      return {
        slow: {
          maxFeePerGas: ethers.formatUnits(baseFee * 90n / 100n, 'gwei'),
          maxPriorityFeePerGas: ethers.formatUnits(priorityFee * 90n / 100n, 'gwei'),
        },
        standard: {
          maxFeePerGas: ethers.formatUnits(baseFee, 'gwei'),
          maxPriorityFeePerGas: ethers.formatUnits(priorityFee, 'gwei'),
        },
        fast: {
          maxFeePerGas: ethers.formatUnits(baseFee * 130n / 100n, 'gwei'),
          maxPriorityFeePerGas: ethers.formatUnits(priorityFee * 130n / 100n, 'gwei'),
        },
      };
    } catch (error) {
      console.error('Error getting EIP-1559 fees:', error);
      throw error;
    }
  }
}