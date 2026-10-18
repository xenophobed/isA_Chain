import { Request, Response } from 'express';
import { IsaNFTService } from '../../services/contracts/isaNFT.service';
import { IPFSService } from '../../services/ipfs.service';
import { logger } from '../../utils/logger';

export async function getTokenDetails(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { tokenId } = req.params;
    const { collection } = req.query;

    const details = await nftService.getTokenInfo(
      collection as string,
      tokenId
    );

    res.json({
      success: true,
      data: details
    });
  } catch (error) {
    logger.error('Failed to get token details:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch token details'
    });
  }
}

export async function getTokenMetadata(nftService: IsaNFTService, ipfs: IPFSService, req: Request, res: Response) {
  try {
    const { tokenId } = req.params;
    const { collection } = req.query;

    const tokenURI = await nftService.getTokenURI(
      collection as string,
      tokenId
    );

    // Fetch metadata from IPFS
    const metadata = await ipfs.getJSON(tokenURI.replace('ipfs://', ''));

    res.json({
      success: true,
      data: {
        tokenId,
        tokenURI,
        metadata
      }
    });
  } catch (error) {
    logger.error('Failed to get token metadata:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch token metadata'
    });
  }
}

export async function getTokenHistory(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { tokenId } = req.params;
    const { collection } = req.query;

    const history = await nftService.getTokenTransferHistory(
      collection as string,
      tokenId
    );

    res.json({
      success: true,
      data: history
    });
  } catch (error) {
    logger.error('Failed to get token history:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch token history'
    });
  }
}

export async function getTokensByOwner(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { address } = req.params;
    const { page = 1, limit = 20 } = req.query;

    const tokens = await nftService.getTokensByOwner(address, {
      page: Number(page),
      limit: Number(limit)
    });

    res.json({
      success: true,
      data: tokens,
      pagination: {
        page: Number(page),
        limit: Number(limit),
        owner: address
      }
    });
  } catch (error) {
    logger.error('Failed to get tokens by owner:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch owner tokens'
    });
  }
}
