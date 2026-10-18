import { Request, Response } from 'express';
import { IsaNFTService } from '../../services/contracts/isaNFT.service';
import { IPFSService } from '../../services/ipfs.service';
import { logger } from '../../utils/logger';

export async function mintNFT(nftService: IsaNFTService, ipfs: IPFSService, req: Request, res: Response) {
  try {
    const { 
      collectionAddress,
      recipient,
      name,
      description,
      attributes = []
    } = req.body;
    
    const file = req.file;
    
    if (!file) {
      return res.status(400).json({
        success: false,
        error: 'Image file is required'
      });
    }

    // Upload image to IPFS
    const imageHash = await ipfs.uploadFile(file.buffer, file.originalname);
    const imageUrl = `ipfs://${imageHash}`;

    // Create and upload metadata
    const metadata = {
      name,
      description,
      image: imageUrl,
      attributes,
      created_at: new Date().toISOString()
    };

    const metadataHash = await ipfs.uploadJSON(metadata);
    const tokenURI = `ipfs://${metadataHash}`;

    // Mint NFT on blockchain
    const txHash = await nftService.mintNFT({
      collectionAddress,
      recipient,
      tokenURI
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        tokenURI,
        imageUrl,
        metadata,
        status: 'pending'
      }
    });
  } catch (error) {
    logger.error('Failed to mint NFT:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to mint NFT'
    });
  }
}

export async function batchMint(nftService: IsaNFTService, ipfs: IPFSService, req: Request, res: Response) {
  try {
    const { collectionAddress, recipients, baseMetadata } = req.body;
    const files = req.files as Express.Multer.File[];

    if (!files || files.length === 0) {
      return res.status(400).json({
        success: false,
        error: 'At least one image file is required'
      });
    }

    const mintData = [];

    for (let i = 0; i < files.length; i++) {
      const file = files[i];
      const recipient = recipients[i] || recipients[0]; // Use first recipient if not enough

      // Upload image to IPFS
      const imageHash = await ipfs.uploadFile(file.buffer, file.originalname);
      
      // Create metadata
      const metadata = {
        ...baseMetadata,
        name: `${baseMetadata.name} #${i + 1}`,
        image: `ipfs://${imageHash}`,
        tokenId: i + 1
      };

      const metadataHash = await ipfs.uploadJSON(metadata);
      
      mintData.push({
        recipient,
        tokenURI: `ipfs://${metadataHash}`
      });
    }

    // Batch mint on blockchain
    const txHash = await nftService.batchMint({
      collectionAddress,
      mintData
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        count: mintData.length,
        tokens: mintData,
        status: 'pending'
      }
    });
  } catch (error) {
    logger.error('Failed to batch mint NFTs:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to batch mint NFTs'
    });
  }
}

export async function lazyMint(nftService: IsaNFTService, req: Request, res: Response) {
  try {
    const { collectionAddress, tokenData, signature } = req.body;

    // Store lazy mint voucher
    const voucher = await nftService.createLazyMintVoucher({
      collectionAddress,
      tokenData,
      signature
    });

    res.json({
      success: true,
      data: {
        voucherId: voucher.id,
        redeemUrl: `${req.protocol}://${req.get('host')}/api/v1/redeem/${voucher.id}`,
        expires: voucher.expires
      }
    });
  } catch (error) {
    logger.error('Failed to create lazy mint:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to create lazy mint voucher'
    });
  }
}
