import { Request, Response } from 'express';
import { IsaNFTService } from '../../services/contracts/isaNFT.service';
import { logger } from '../../utils/logger';

export async function getCollections(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { page = 1, limit = 20, sortBy = 'created' } = req.query;
    
    const collections = await nftService.getAllCollections({
      page: Number(page),
      limit: Number(limit),
      sortBy: sortBy as string
    });

    res.json({
      success: true,
      data: collections,
      pagination: {
        page: Number(page),
        limit: Number(limit),
        total: collections.length
      }
    });
  } catch (error) {
    logger.error('Failed to get collections:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch collections'
    });
  }
}

export async function createCollection(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { name, symbol, baseURI, maxSupply, royaltyBps } = req.body;

    const txHash = await nftService.deployCollection({
      name,
      symbol,
      baseURI,
      maxSupply,
      royaltyBps: royaltyBps || 250 // 2.5% default royalty
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Collection deployment initiated'
      }
    });
  } catch (error) {
    logger.error('Failed to create collection:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to create collection'
    });
  }
}

export async function getCollectionDetails(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { address } = req.params;
    
    const details = await nftService.getCollectionInfo(address);
    
    res.json({
      success: true,
      data: details
    });
  } catch (error) {
    logger.error('Failed to get collection details:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch collection details'
    });
  }
}

export async function getCollectionStats(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { address } = req.params;
    
    const stats = await nftService.getCollectionStatistics(address);
    
    res.json({
      success: true,
      data: stats
    });
  } catch (error) {
    logger.error('Failed to get collection stats:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch collection statistics'
    });
  }
}
