import { ethers } from 'ethers';
import { config } from '../config';
import { logger } from '../utils/logger';

export class BlockchainService {
  private static instance: BlockchainService;
  private provider: ethers.JsonRpcProvider;
  private signer: ethers.Wallet;

  private constructor() {
    // Initialize provider
    this.provider = new ethers.JsonRpcProvider(config.blockchain.rpcUrl);
    
    // Initialize signer with private key
    this.signer = new ethers.Wallet(config.blockchain.privateKey, this.provider);
    
    logger.info(`Connected to blockchain at ${config.blockchain.rpcUrl}`);
    logger.info(`Using address: ${this.signer.address}`);
  }

  static async getInstance(): Promise<BlockchainService> {
    if (!BlockchainService.instance) {
      BlockchainService.instance = new BlockchainService();
      
      // Test connection
      try {
        const network = await BlockchainService.instance.provider.getNetwork();
        logger.info(`Connected to network: ${network.name} (chainId: ${network.chainId})`);
      } catch (error) {
        logger.error('Failed to connect to blockchain:', error);
        throw error;
      }
    }
    
    return BlockchainService.instance;
  }

  getProvider(): ethers.JsonRpcProvider {
    return this.provider;
  }

  getSigner(): ethers.Wallet {
    return this.signer;
  }

  async getContract(address: string, abi: any[]): Promise<ethers.Contract> {
    return new ethers.Contract(address, abi, this.signer);
  }

  async sendTransaction(tx: ethers.TransactionRequest): Promise<string> {
    try {
      const response = await this.signer.sendTransaction(tx);
      logger.info(`Transaction sent: ${response.hash}`);
      
      // Wait for confirmation
      const receipt = await response.wait();
      logger.info(`Transaction confirmed: ${receipt?.hash}`);
      
      return response.hash;
    } catch (error) {
      logger.error('Transaction failed:', error);
      throw error;
    }
  }

  async callContract(
    contractAddress: string,
    abi: any[],
    method: string,
    params: any[]
  ): Promise<any> {
    try {
      const contract = await this.getContract(contractAddress, abi);
      const result = await contract[method](...params);
      return result;
    } catch (error) {
      logger.error(`Contract call failed: ${method}`, error);
      throw error;
    }
  }

  async estimateGas(tx: ethers.TransactionRequest): Promise<bigint> {
    return await this.provider.estimateGas(tx);
  }

  async getGasPrice(): Promise<bigint> {
    const feeData = await this.provider.getFeeData();
    return feeData.gasPrice || BigInt(0);
  }

  async getBalance(address: string): Promise<bigint> {
    return await this.provider.getBalance(address);
  }

  async getBlockNumber(): Promise<number> {
    return await this.provider.getBlockNumber();
  }

  async getTransaction(hash: string): Promise<ethers.TransactionResponse | null> {
    return await this.provider.getTransaction(hash);
  }

  async waitForTransaction(hash: string, confirmations = 1): Promise<ethers.TransactionReceipt | null> {
    return await this.provider.waitForTransaction(hash, confirmations);
  }
}