import { ethers } from 'ethers';
import { logger } from '../utils/logger';

export class BlockchainService {
    private provider: ethers.JsonRpcProvider;
    private signer: ethers.Signer;
    private static instance: BlockchainService;

    private constructor() {
        this.provider = new ethers.JsonRpcProvider(
            process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545'
        );

        // Setup signer
        if (process.env.PRIVATE_KEY) {
            this.signer = new ethers.Wallet(process.env.PRIVATE_KEY, this.provider);
        } else {
            this.signer = this.provider.getSigner();
        }
    }

    static async getInstance(): Promise<BlockchainService> {
        if (!BlockchainService.instance) {
            BlockchainService.instance = new BlockchainService();
            await BlockchainService.instance.initialize();
        }
        return BlockchainService.instance;
    }

    private async initialize() {
        logger.info('Blockchain service initialized');
        logger.info(`Connected to RPC: ${process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545'}`);
    }

    async getBlockNumber(): Promise<number> {
        try {
            return await this.provider.getBlockNumber();
        } catch (error) {
            logger.error('Failed to get block number:', error);
            throw error;
        }
    }

    async getBalance(address: string): Promise<string> {
        try {
            const balance = await this.provider.getBalance(address);
            return ethers.utils.formatEther(balance);
        } catch (error) {
            logger.error('Failed to get balance:', error);
            throw error;
        }
    }

    async getTransaction(hash: string): Promise<any> {
        try {
            return await this.provider.getTransaction(hash);
        } catch (error) {
            logger.error('Failed to get transaction:', error);
            throw error;
        }
    }

    async waitForTransaction(hash: string): Promise<any> {
        try {
            return await this.provider.waitForTransaction(hash);
        } catch (error) {
            logger.error('Failed to wait for transaction:', error);
            throw error;
        }
    }

    getProvider(): ethers.JsonRpcProvider {
        return this.provider;
    }

    getSigner(): ethers.Signer {
        return this.signer;
    }
}

// Export singleton instance
export const blockchainService = new BlockchainService();