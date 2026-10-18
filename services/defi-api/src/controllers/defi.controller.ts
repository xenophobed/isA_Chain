import { Request, Response } from 'express';
import { BlockchainService } from '../services/blockchain.service';
import { SimpleDEXService } from '../services/contracts/simpleDEX.service';
import { StakingPoolService } from '../services/contracts/stakingPool.service';
import { YieldFarmingService } from '../services/contracts/yieldFarming.service';
import { LendingProtocolService } from '../services/contracts/lendingProtocol.service';
import * as dexHandler from './defi/dex.handler';
import * as stakingHandler from './defi/staking.handler';
import * as yieldHandler from './defi/yield.handler';
import * as lendingHandler from './defi/lending.handler';

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
    return dexHandler.getPools(this.simpleDEX, req, res);
  }

  async getSwapQuote(req: Request, res: Response) {
    return dexHandler.getSwapQuote(this.simpleDEX, req, res);
  }

  async executeSwap(req: Request, res: Response) {
    return dexHandler.executeSwap(this.simpleDEX, req, res);
  }

  async addLiquidity(req: Request, res: Response) {
    return dexHandler.addLiquidity(this.simpleDEX, req, res);
  }

  async removeLiquidity(req: Request, res: Response) {
    return dexHandler.removeLiquidity(this.simpleDEX, req, res);
  }

  async getLiquidityPositions(req: Request, res: Response) {
    return dexHandler.getLiquidityPositions(this.simpleDEX, req, res);
  }

  // ============ Staking Functions ============

  async stake(req: Request, res: Response) {
    return stakingHandler.stake(this.stakingPool, req, res);
  }

  async unstake(req: Request, res: Response) {
    return stakingHandler.unstake(this.stakingPool, req, res);
  }

  async getStakingRewards(req: Request, res: Response) {
    return stakingHandler.getStakingRewards(this.stakingPool, req, res);
  }

  async claimRewards(req: Request, res: Response) {
    return stakingHandler.claimRewards(this.stakingPool, req, res);
  }

  // ============ Yield Farming Functions ============

  async getFarms(req: Request, res: Response) {
    return yieldHandler.getFarms(this.yieldFarming, req, res);
  }

  async depositToFarm(req: Request, res: Response) {
    return yieldHandler.depositToFarm(this.yieldFarming, req, res);
  }

  async withdrawFromFarm(req: Request, res: Response) {
    return yieldHandler.withdrawFromFarm(this.yieldFarming, req, res);
  }

  async harvestYield(req: Request, res: Response) {
    return yieldHandler.harvestYield(this.yieldFarming, req, res);
  }

  // ============ Lending Functions ============

  async getLendingMarkets(req: Request, res: Response) {
    return lendingHandler.getLendingMarkets(this.lendingProtocol, req, res);
  }

  async supplyAsset(req: Request, res: Response) {
    return lendingHandler.supplyAsset(this.lendingProtocol, req, res);
  }

  async borrowAsset(req: Request, res: Response) {
    return lendingHandler.borrowAsset(this.lendingProtocol, req, res);
  }

  async repayLoan(req: Request, res: Response) {
    return lendingHandler.repayLoan(this.lendingProtocol, req, res);
  }

  async getLendingPosition(req: Request, res: Response) {
    return lendingHandler.getLendingPosition(this.lendingProtocol, req, res);
  }
}