import { Request, Response } from 'express';
import { YieldFarmingService } from '../../services/contracts/yieldFarming.service';
import { logger } from '../../utils/logger';

export async function getFarms(yieldFarming: YieldFarmingService, req: Request, res: Response) {
  try {
    const farms = await yieldFarming.getAllFarms();
    
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

export async function depositToFarm(yieldFarming: YieldFarmingService, req: Request, res: Response) {
  try {
    const { farmId, amount, userAddress } = req.body;

    const txHash = await yieldFarming.deposit({
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

export async function withdrawFromFarm(yieldFarming: YieldFarmingService, req: Request, res: Response) {
  try {
    const { farmId, amount, userAddress } = req.body;

    const txHash = await yieldFarming.withdraw({
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

export async function harvestYield(yieldFarming: YieldFarmingService, req: Request, res: Response) {
  try {
    const { farmId, userAddress } = req.body;

    const txHash = await yieldFarming.harvest({
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
