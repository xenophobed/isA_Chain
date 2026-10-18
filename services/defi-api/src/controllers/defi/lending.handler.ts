import { Request, Response } from 'express';
import { LendingProtocolService } from '../../services/contracts/lendingProtocol.service';
import { logger } from '../../utils/logger';

export async function getLendingMarkets(lendingProtocol: LendingProtocolService, req: Request, res: Response) {
  try {
    const markets = await lendingProtocol.getMarkets();
    
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

export async function supplyAsset(lendingProtocol: LendingProtocolService, req: Request, res: Response) {
  try {
    const { asset, amount, userAddress } = req.body;

    const txHash = await lendingProtocol.supply({
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

export async function borrowAsset(lendingProtocol: LendingProtocolService, req: Request, res: Response) {
  try {
    const { asset, amount, userAddress } = req.body;

    const txHash = await lendingProtocol.borrow({
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

export async function repayLoan(lendingProtocol: LendingProtocolService, req: Request, res: Response) {
  try {
    const { asset, amount, userAddress } = req.body;

    const txHash = await lendingProtocol.repay({
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

export async function getLendingPosition(lendingProtocol: LendingProtocolService, req: Request, res: Response) {
  try {
    const { address } = req.params;
    
    const position = await lendingProtocol.getUserPosition(address);
    
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
