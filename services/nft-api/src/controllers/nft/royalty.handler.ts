import { Request, Response } from 'express';
import { IsaNFTService } from '../../services/contracts/isaNFT.service';
import { logger } from '../../utils/logger';

export async function setRoyalty(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collection, recipient, percentage } = req.body;

    const txHash = await nftService.setRoyalty({
      collection,
      recipient,
      percentage: percentage * 100 // Convert to basis points
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Royalty configuration updated'
      }
    });
  } catch (error) {
    logger.error('Failed to set royalty:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to set royalty'
    });
  }
}

export async function getRoyaltyInfo(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collection } = req.params;

    const royaltyInfo = await nftService.getRoyaltyInfo(collection);

    res.json({
      success: true,
      data: royaltyInfo
    });
  } catch (error) {
    logger.error('Failed to get royalty info:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch royalty information'
    });
  }
}

export async function claimRoyalties(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collection, recipient } = req.body;

    const txHash = await nftService.claimRoyalties({
      collection,
      recipient
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Royalty claim initiated'
      }
    });
  } catch (error) {
    logger.error('Failed to claim royalties:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to claim royalties'
    });
  }
}
