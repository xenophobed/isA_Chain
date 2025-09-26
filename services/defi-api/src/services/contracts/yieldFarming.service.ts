import { ethers } from 'ethers';
import { BlockchainService } from '../blockchain.service';
import { logger } from '../../utils/logger';
import { config } from '../../config';

interface FarmParams {
  poolId: number;
  amount: string;
  userAddress: string;
}

interface HarvestParams {
  poolId: number;
  userAddress: string;
}

interface WithdrawParams {
  poolId: number;
  amount: string;
  userAddress: string;
}

export class YieldFarmingService {
  private contract: ethers.Contract | null = null;
  private abi: any[] = []; // Placeholder ABI

  constructor(private blockchain: BlockchainService) {}

  private async getContract(): Promise<ethers.Contract> {
    if (!this.contract) {
      this.contract = await this.blockchain.getContract(
        config.contracts.yieldFarming,
        this.abi
      );
    }
    return this.contract;
  }

  async getAllFarms() {
    try {
      const contract = await this.getContract();
      
      const poolCount = await contract.poolLength();
      const farms = [];
      
      for (let i = 0; i < poolCount; i++) {
        const poolInfo = await contract.poolInfo(i);
        const totalStaked = await contract.totalStaked(i);
        
        farms.push({
          poolId: i,
          lpToken: poolInfo.lpToken,
          allocPoint: poolInfo.allocPoint.toString(),
          lastRewardBlock: poolInfo.lastRewardBlock.toString(),
          accRewardPerShare: poolInfo.accRewardPerShare.toString(),
          totalStaked: ethers.formatEther(totalStaked),
          apr: await this.calculatePoolAPR(i),
          rewardToken: await contract.rewardToken()
        });
      }
      
      return farms;
    } catch (error) {
      logger.error('Failed to get all farms:', error);
      throw error;
    }
  }

  async getUserFarmInfo(poolId: number, address: string) {
    try {
      const contract = await this.getContract();
      
      const userInfo = await contract.userInfo(poolId, address);
      const pendingRewards = await contract.pendingReward(poolId, address);
      
      return {
        amount: ethers.formatEther(userInfo.amount),
        rewardDebt: ethers.formatEther(userInfo.rewardDebt),
        pendingRewards: ethers.formatEther(pendingRewards),
        depositTime: userInfo.depositTime?.toString() || null
      };
    } catch (error) {
      logger.error('Failed to get user farm info:', error);
      throw error;
    }
  }

  async deposit(params: FarmParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.deposit(
        params.poolId,
        ethers.parseEther(params.amount),
        { from: params.userAddress }
      );
      
      logger.info(`Farm deposit transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to deposit to farm:', error);
      throw error;
    }
  }

  async withdraw(params: WithdrawParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.withdraw(
        params.poolId,
        ethers.parseEther(params.amount),
        { from: params.userAddress }
      );
      
      logger.info(`Farm withdraw transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to withdraw from farm:', error);
      throw error;
    }
  }

  async harvest(params: HarvestParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.harvest(
        params.poolId,
        { from: params.userAddress }
      );
      
      logger.info(`Farm harvest transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to harvest from farm:', error);
      throw error;
    }
  }

  async harvestAll(userAddress: string): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.harvestAll({
        from: userAddress
      });
      
      logger.info(`Farm harvest all transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to harvest all from farms:', error);
      throw error;
    }
  }

  async emergencyWithdraw(poolId: number, userAddress: string): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.emergencyWithdraw(
        poolId,
        { from: userAddress }
      );
      
      logger.info(`Farm emergency withdraw transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to emergency withdraw from farm:', error);
      throw error;
    }
  }

  async getFarmingStats() {
    try {
      const contract = await this.getContract();
      
      const totalAllocPoint = await contract.totalAllocPoint();
      const rewardPerBlock = await contract.rewardPerBlock();
      const poolCount = await contract.poolLength();
      
      return {
        totalAllocPoint: totalAllocPoint.toString(),
        rewardPerBlock: ethers.formatEther(rewardPerBlock),
        poolCount: Number(poolCount),
        totalValueLocked: await this.calculateTotalTVL()
      };
    } catch (error) {
      logger.error('Failed to get farming stats:', error);
      throw error;
    }
  }

  async getUserAllFarms(address: string) {
    try {
      const contract = await this.getContract();
      const poolCount = await contract.poolLength();
      const userFarms = [];
      
      for (let i = 0; i < poolCount; i++) {
        const userInfo = await contract.userInfo(i, address);
        
        if (userInfo.amount > 0) {
          const pendingRewards = await contract.pendingReward(i, address);
          const poolInfo = await contract.poolInfo(i);
          
          userFarms.push({
            poolId: i,
            lpToken: poolInfo.lpToken,
            stakedAmount: ethers.formatEther(userInfo.amount),
            pendingRewards: ethers.formatEther(pendingRewards),
            apr: await this.calculatePoolAPR(i)
          });
        }
      }
      
      return userFarms;
    } catch (error) {
      logger.error('Failed to get user all farms:', error);
      throw error;
    }
  }

  private async calculatePoolAPR(poolId: number): Promise<number> {
    try {
      const contract = await this.getContract();
      
      const poolInfo = await contract.poolInfo(poolId);
      const totalAllocPoint = await contract.totalAllocPoint();
      const rewardPerBlock = await contract.rewardPerBlock();
      const totalStaked = await contract.totalStaked(poolId);
      
      if (totalStaked === BigInt(0) || totalAllocPoint === BigInt(0)) {
        return 0;
      }
      
      // Calculate APR (simplified)
      const poolRewardPerBlock = (Number(rewardPerBlock) * Number(poolInfo.allocPoint)) / Number(totalAllocPoint);
      const blocksPerYear = 365 * 24 * 60 * 60 / 12; // Assuming 12 second blocks
      const yearlyRewards = poolRewardPerBlock * blocksPerYear;
      const apr = (yearlyRewards / Number(totalStaked)) * 100;
      
      return Math.round(apr * 100) / 100;
    } catch (error) {
      logger.error('Failed to calculate pool APR:', error);
      return 0;
    }
  }

  private async calculateTotalTVL(): Promise<string> {
    try {
      const contract = await this.getContract();
      const poolCount = await contract.poolLength();
      let totalTVL = BigInt(0);
      
      for (let i = 0; i < poolCount; i++) {
        const totalStaked = await contract.totalStaked(i);
        totalTVL += totalStaked;
      }
      
      return ethers.formatEther(totalTVL);
    } catch (error) {
      logger.error('Failed to calculate total TVL:', error);
      return '0';
    }
  }
}