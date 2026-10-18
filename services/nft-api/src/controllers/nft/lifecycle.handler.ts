import { Request, Response } from 'express';
import { IsaNFTService } from '../../services/contracts/isaNFT.service';
import { IPFSService } from '../../services/ipfs.service';
import { logger } from '../../utils/logger';

export async function transferNFT(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collection, tokenId, from, to } = req.body;

    const txHash = await nftService.transferNFT({
      collection,
      tokenId,
      from,
      to
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'NFT transfer initiated'
      }
    });
  } catch (error) {
    logger.error('Failed to transfer NFT:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to transfer NFT'
    });
  }
}

export async function burnNFT(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collection, tokenId, owner } = req.body;

    const txHash = await nftService.burnNFT({
      collection,
      tokenId,
      owner
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'NFT burn initiated'
      }
    });
  } catch (error) {
    logger.error('Failed to burn NFT:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to burn NFT'
    });
  }
}

export async function approveNFT(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collection, tokenId, operator, owner } = req.body;

    const txHash = await nftService.approve({
      collection,
      tokenId,
      operator,
      owner
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Approval granted'
      }
    });
  } catch (error) {
    logger.error('Failed to approve NFT:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to approve NFT'
    });
  }
}

export async function updateMetadata(nftService: IsaNFTService, ipfs: IPFSService, req: Request, res: Response) {
  try {
    const { collection, tokenId, metadata } = req.body;

    // Upload new metadata to IPFS
    const metadataHash = await ipfs.uploadJSON(metadata);
    const newTokenURI = `ipfs://${metadataHash}`;

    // Update on blockchain
    const txHash = await nftService.updateTokenURI({
      collection,
      tokenId,
      newTokenURI
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        newTokenURI,
        metadata,
        status: 'pending'
      }
    });
  } catch (error) {
    logger.error('Failed to update metadata:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to update metadata'
    });
  }
}
