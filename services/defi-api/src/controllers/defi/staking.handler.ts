import { Request, Response } from 'express';
import { StakingPoolService } from '../../services/contracts/stakingPool.service';
import { logger } from '../../utils/logger';

export async function stake(stakingPool: StakingPoolService, req: Request, res: Response) {
  try {
    const { amount, duration, userAddress } = req.body;

    const txHash = await stakingPool.stake({
      amount,
      duration,
      userAddress
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Staking transaction submitted'
      }
    });
  } catch (error) {
    logger.error('Failed to stake:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to stake tokens'
    });
  }
}

export async function unstake(stakingPool: StakingPoolService, req: Request, res: Response) {
  try {
    const { amount, userAddress } = req.body;

    const txHash = await stakingPool.unstake({
      amount,
      userAddress
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Unstaking transaction submitted'
      }
    });
  } catch (error) {
    logger.error('Failed to unstake:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to unstake tokens'
    });
  }
}

export async function getStakingRewards(stakingPool: StakingPoolService, req: Request, res: Response) {
  try {
    const { address } = req.params;
    
    const rewards = await stakingPool.getPendingRewards(address);
    
    res.json({
      success: true,
      data: {
        pendingRewards: rewards.pending,
        claimedRewards: rewards.claimed,
        apr: rewards.apr,
        stakedAmount: rewards.stakedAmount
      }
    });
  } catch (error) {
    logger.error('Failed to get staking rewards:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch staking rewards'
    });
  }
}

export async function claimRewards(stakingPool: StakingPoolService, req: Request, res: Response) {
  try {
    const { userAddress } = req.body;

    const txHash = await stakingPool.claimRewards(userAddress);

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Rewards claim transaction submitted'
      }
    });
  } catch (error) {
    logger.error('Failed to claim rewards:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to claim rewards'
    });
  }
}
