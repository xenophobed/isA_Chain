import { ethers } from 'ethers';
import { BlockchainService } from '../blockchain.service';
import { logger } from '../../utils/logger';
import { config } from '../../config';

interface StakeParams {
  amount: string;
  userAddress: string;
}

interface UnstakeParams {
  amount: string;
  userAddress: string;
}

interface ClaimParams {
  userAddress: string;
}

export class StakingPoolService {
  private contract: ethers.Contract | null = null;
  private abi: any[] = []; // Placeholder ABI

  constructor(private blockchain: BlockchainService) {}

  private async getContract(): Promise<ethers.Contract> {
    if (!this.contract) {
      this.contract = await this.blockchain.getContract(
        config.contracts.stakingPool,
        this.abi
      );
    }
    return this.contract;
  }

  async getPoolInfo() {
    try {
      const contract = await this.getContract();
      
      // Get basic pool information
      const totalStaked = await contract.totalStaked();
      const rewardRate = await contract.rewardRate();
      const stakingToken = await contract.stakingToken();
      const rewardToken = await contract.rewardToken();
      
      return {
        totalStaked: ethers.formatEther(totalStaked),
        rewardRate: ethers.formatEther(rewardRate),
        stakingToken,
        rewardToken,
        apr: await this.calculateAPR()
      };
    } catch (error) {
      logger.error('Failed to get pool info:', error);
      throw error;
    }
  }

  async getUserStakingInfo(address: string) {
    try {
      const contract = await this.getContract();
      
      const userStake = await contract.userStakes(address);
      const earnedRewards = await contract.earned(address);
      
      return {
        stakedAmount: ethers.formatEther(userStake.amount),
        earnedRewards: ethers.formatEther(earnedRewards),
        stakingTimestamp: userStake.timestamp.toString(),
        lockEndTime: userStake.lockEndTime?.toString() || null
      };
    } catch (error) {
      logger.error('Failed to get user staking info:', error);
      throw error;
    }
  }

  async stake(params: StakeParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.stake(
        ethers.parseEther(params.amount),
        { from: params.userAddress }
      );
      
      logger.info(`Stake transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to stake:', error);
      throw error;
    }
  }

  async unstake(params: UnstakeParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.unstake(
        ethers.parseEther(params.amount),
        { from: params.userAddress }
      );
      
      logger.info(`Unstake transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to unstake:', error);
      throw error;
    }
  }

  async claimRewards(params: ClaimParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.claimRewards({
        from: params.userAddress
      });
      
      logger.info(`Claim rewards transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to claim rewards:', error);
      throw error;
    }
  }

  async getStakingRewards(address: string) {
    try {
      const contract = await this.getContract();
      
      const earned = await contract.earned(address);
      const rewardRate = await contract.rewardRate();
      const userStake = await contract.userStakes(address);
      
      // Calculate estimated daily rewards
      const dailyRewards = this.calculateDailyRewards(
        userStake.amount,
        rewardRate
      );
      
      return {
        earned: ethers.formatEther(earned),
        dailyRewards,
        rewardRate: ethers.formatEther(rewardRate)
      };
    } catch (error) {
      logger.error('Failed to get staking rewards:', error);
      throw error;
    }
  }

  async emergencyWithdraw(userAddress: string): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.emergencyWithdraw({
        from: userAddress
      });
      
      logger.info(`Emergency withdraw transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to emergency withdraw:', error);
      throw error;
    }
  }

  private async calculateAPR(): Promise<number> {
    try {
      const contract = await this.getContract();
      
      const rewardRate = await contract.rewardRate();
      const totalStaked = await contract.totalStaked();
      
      if (totalStaked === BigInt(0)) {
        return 0;
      }
      
      // Calculate APR (simplified)
      const yearlyRewards = Number(rewardRate) * 365 * 24 * 3600;
      const apr = (yearlyRewards / Number(totalStaked)) * 100;
      
      return Math.round(apr * 100) / 100;
    } catch (error) {
      logger.error('Failed to calculate APR:', error);
      return 0;
    }
  }

  private calculateDailyRewards(stakedAmount: bigint, rewardRate: bigint): string {
    const dailyRate = Number(rewardRate) * 24 * 3600;
    const userShare = Number(stakedAmount);
    const dailyRewards = (userShare * dailyRate) / 1e18;
    
    return dailyRewards.toFixed(6);
  }
}