import { Request, Response } from 'express';
import { BlockchainService } from '../services/blockchain.service';
import { SimpleDEXService } from '../services/contracts/simpleDEX.service';
import { StakingPoolService } from '../services/contracts/stakingPool.service';
import { YieldFarmingService } from '../services/contracts/yieldFarming.service';
import { LendingProtocolService } from '../services/contracts/lendingProtocol.service';
import { logger } from '../utils/logger';

export class DeFiController {
  private simpleDEX: SimpleDEXService;
  private stakingPool: StakingPoolService;
  private yieldFarming: YieldFarmingService;
  private lendingProtocol: LendingProtocolService;

  constructor(private blockchain: BlockchainService) {
    this.simpleDEX = new SimpleDEXService(blockchain);
    this.stakingPool = new StakingPoolService(blockchain);
    this.yieldFarming = new YieldFarmingService(blockchain);
    this.lendingProtocol = new LendingProtocolService(blockchain);
  }

  // ============ DEX Functions ============

  async getPools(req: Request, res: Response) {
    try {
      const pools = await this.simpleDEX.getAllPools();
      res.json({
        success: true,
        data: pools,
        timestamp: Date.now()
      });
    } catch (error) {
      logger.error('Failed to get pools:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to fetch pools'
      });
    }
  }

  async getSwapQuote(req: Request, res: Response) {
    try {
      const { tokenIn, tokenOut, amountIn } = req.body;
      
      if (!tokenIn || !tokenOut || !amountIn) {
        return res.status(400).json({
          success: false,
          error: 'Missing required parameters: tokenIn, tokenOut, amountIn'
        });
      }

      const quote = await this.simpleDEX.getSwapQuote(tokenIn, tokenOut, amountIn);
      
      res.json({
        success: true,
        data: {
          tokenIn,
          tokenOut,
          amountIn,
          estimatedOut: quote.estimatedOut,
          priceImpact: quote.priceImpact,
          fee: quote.fee,
          route: quote.route
        }
      });
    } catch (error) {
      logger.error('Failed to get swap quote:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to calculate swap quote'
      });
    }
  }

  async executeSwap(req: Request, res: Response) {
    try {
      const { tokenIn, tokenOut, amountIn, minAmountOut, userAddress } = req.body;
      
      if (!tokenIn || !tokenOut || !amountIn || !minAmountOut || !userAddress) {
        return res.status(400).json({
          success: false,
          error: 'Missing required parameters'
        });
      }

      const txHash = await this.simpleDEX.executeSwap({
        tokenIn,
        tokenOut,
        amountIn,
        minAmountOut,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Swap transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to execute swap:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to execute swap'
      });
    }
  }

  async addLiquidity(req: Request, res: Response) {
    try {
      const { tokenA, tokenB, amountA, amountB, userAddress } = req.body;

      const txHash = await this.simpleDEX.addLiquidity({
        tokenA,
        tokenB,
        amountA,
        amountB,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Liquidity addition transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to add liquidity:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to add liquidity'
      });
    }
  }

  async removeLiquidity(req: Request, res: Response) {
    try {
      const { poolId, lpTokenAmount, userAddress } = req.body;

      const txHash = await this.simpleDEX.removeLiquidity({
        poolId,
        lpTokenAmount,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Liquidity removal transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to remove liquidity:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to remove liquidity'
      });
    }
  }

  async getLiquidityPositions(req: Request, res: Response) {
    try {
      const { address } = req.params;
      
      const positions = await this.simpleDEX.getUserLiquidityPositions(address);
      
      res.json({
        success: true,
        data: positions
      });
    } catch (error) {
      logger.error('Failed to get liquidity positions:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to fetch liquidity positions'
      });
    }
  }

  // ============ Staking Functions ============

  async stake(req: Request, res: Response) {
    try {
      const { amount, duration, userAddress } = req.body;

      const txHash = await this.stakingPool.stake({
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

  async unstake(req: Request, res: Response) {
    try {
      const { amount, userAddress } = req.body;

      const txHash = await this.stakingPool.unstake({
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

  async getStakingRewards(req: Request, res: Response) {
    try {
      const { address } = req.params;
      
      const rewards = await this.stakingPool.getPendingRewards(address);
      
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

  async claimRewards(req: Request, res: Response) {
    try {
      const { userAddress } = req.body;

      const txHash = await this.stakingPool.claimRewards(userAddress);

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

  // ============ Yield Farming Functions ============

  async getFarms(req: Request, res: Response) {
    try {
      const farms = await this.yieldFarming.getAllFarms();
      
      res.json({
        success: true,
        data: farms
      });
    } catch (error) {
      logger.error('Failed to get farms:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to fetch farms'
      });
    }
  }

  async depositToFarm(req: Request, res: Response) {
    try {
      const { farmId, amount, userAddress } = req.body;

      const txHash = await this.yieldFarming.deposit({
        farmId,
        amount,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Farm deposit transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to deposit to farm:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to deposit to farm'
      });
    }
  }

  async withdrawFromFarm(req: Request, res: Response) {
    try {
      const { farmId, amount, userAddress } = req.body;

      const txHash = await this.yieldFarming.withdraw({
        farmId,
        amount,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Farm withdrawal transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to withdraw from farm:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to withdraw from farm'
      });
    }
  }

  async harvestYield(req: Request, res: Response) {
    try {
      const { farmId, userAddress } = req.body;

      const txHash = await this.yieldFarming.harvest({
        farmId,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Harvest transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to harvest yield:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to harvest yield'
      });
    }
  }

  // ============ Lending Functions ============

  async getLendingMarkets(req: Request, res: Response) {
    try {
      const markets = await this.lendingProtocol.getMarkets();
      
      res.json({
        success: true,
        data: markets
      });
    } catch (error) {
      logger.error('Failed to get lending markets:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to fetch lending markets'
      });
    }
  }

  async supplyAsset(req: Request, res: Response) {
    try {
      const { asset, amount, userAddress } = req.body;

      const txHash = await this.lendingProtocol.supply({
        asset,
        amount,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Supply transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to supply asset:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to supply asset'
      });
    }
  }

  async borrowAsset(req: Request, res: Response) {
    try {
      const { asset, amount, userAddress } = req.body;

      const txHash = await this.lendingProtocol.borrow({
        asset,
        amount,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Borrow transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to borrow asset:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to borrow asset'
      });
    }
  }

  async repayLoan(req: Request, res: Response) {
    try {
      const { asset, amount, userAddress } = req.body;

      const txHash = await this.lendingProtocol.repay({
        asset,
        amount,
        userAddress
      });

      res.json({
        success: true,
        data: {
          transactionHash: txHash,
          status: 'pending',
          message: 'Repay transaction submitted'
        }
      });
    } catch (error) {
      logger.error('Failed to repay loan:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to repay loan'
      });
    }
  }

  async getLendingPosition(req: Request, res: Response) {
    try {
      const { address } = req.params;
      
      const position = await this.lendingProtocol.getUserPosition(address);
      
      res.json({
        success: true,
        data: position
      });
    } catch (error) {
      logger.error('Failed to get lending position:', error);
      res.status(500).json({
        success: false,
        error: 'Failed to fetch lending position'
      });
    }
  }
}