import { Request, Response } from 'express';
import { BlockchainService } from '../services/blockchain.service';
import { IPFSService } from '../services/ipfs.service';
import { IsaNFTService } from '../services/contracts/isaNFT.service';
import { logger } from '../utils/logger';

export class NFTController {
  private nftService: IsaNFTService;

  constructor(
    private blockchain: BlockchainService,
    private ipfs: IPFSService
  ) {
    this.nftService = new IsaNFTService();
  }

  // ============ Collection Management ============

  async getCollections(req: Request, res: Response) {
    try {
      const { page = 1, limit = 20, sortBy = 'created' } = req.query;
      
      const collections = await this.nftService.getAllCollections({
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

  async createCollection(req: Request, res: Response) {
    try {
      const { name, symbol, baseURI, maxSupply, royaltyBps } = req.body;

      const txHash = await this.nftService.deployCollection({
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

  async getCollectionDetails(req: Request, res: Response) {
    try {
      const { address } = req.params;
      
      const details = await this.nftService.getCollectionInfo(address);
      
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

  async getCollectionStats(req: Request, res: Response) {
    try {
      const { address } = req.params;
      
      const stats = await this.nftService.getCollectionStatistics(address);
      
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

  // ============ NFT Minting ============

  async mintNFT(req: Request, res: Response) {
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
      const imageHash = await this.ipfs.uploadFile(file.buffer, file.originalname);
      const imageUrl = `ipfs://${imageHash}`;

      // Create and upload metadata
      const metadata = {
        name,
        description,
        image: imageUrl,
        attributes,
        created_at: new Date().toISOString()
      };

      const metadataHash = await this.ipfs.uploadJSON(metadata);
      const tokenURI = `ipfs://${metadataHash}`;

      // Mint NFT on blockchain
      const txHash = await this.nftService.mintNFT({
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

  async batchMint(req: Request, res: Response) {
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
        const imageHash = await this.ipfs.uploadFile(file.buffer, file.originalname);
        
        // Create metadata
        const metadata = {
          ...baseMetadata,
          name: `${baseMetadata.name} #${i + 1}`,
          image: `ipfs://${imageHash}`,
          tokenId: i + 1
        };

        const metadataHash = await this.ipfs.uploadJSON(metadata);
        
        mintData.push({
          recipient,
          tokenURI: `ipfs://${metadataHash}`
        });
      }

      // Batch mint on blockchain
      const txHash = await this.nftService.batchMint({
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

  async lazyMint(req: Request, res: Response) {
    try {
      const { collectionAddress, tokenData, signature } = req.body;

      // Store lazy mint voucher
      const voucher = await this.nftService.createLazyMintVoucher({
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

  // ============ NFT Queries ============

  async getTokenDetails(req: Request, res: Response) {
    try {
      const { tokenId } = req.params;
      const { collection } = req.query;

      const details = await this.nftService.getTokenInfo(
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

  async getTokenMetadata(req: Request, res: Response) {
    try {
      const { tokenId } = req.params;
      const { collection } = req.query;

      const tokenURI = await this.nftService.getTokenURI(
        collection as string,
        tokenId
      );

      // Fetch metadata from IPFS
      const metadata = await this.ipfs.getJSON(tokenURI.replace('ipfs://', ''));

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

  async getTokenHistory(req: Request, res: Response) {
    try {
      const { tokenId } = req.params;
      const { collection } = req.query;

      const history = await this.nftService.getTokenTransferHistory(
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

  async getTokensByOwner(req: Request, res: Response) {
    try {
      const { address } = req.params;
      const { page = 1, limit = 20 } = req.query;

      const tokens = await this.nftService.getTokensByOwner(address, {
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

  // ============ NFT Management ============

  async transferNFT(req: Request, res: Response) {
    try {
      const { collection, tokenId, from, to } = req.body;

      const txHash = await this.nftService.transferNFT({
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

  async burnNFT(req: Request, res: Response) {
    try {
      const { collection, tokenId, owner } = req.body;

      const txHash = await this.nftService.burnNFT({
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

  async approveNFT(req: Request, res: Response) {
    try {
      const { collection, tokenId, operator, owner } = req.body;

      const txHash = await this.nftService.approve({
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

  async updateMetadata(req: Request, res: Response) {
    try {
      const { collection, tokenId, metadata } = req.body;

      // Upload new metadata to IPFS
      const metadataHash = await this.ipfs.uploadJSON(metadata);
      const newTokenURI = `ipfs://${metadataHash}`;

      // Update on blockchain
      const txHash = await this.nftService.updateTokenURI({
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

  // ============ Royalty Management ============

  async setRoyalty(req: Request, res: Response) {
    try {
      const { collection, recipient, percentage } = req.body;

      const txHash = await this.nftService.setRoyalty({
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

  async getRoyaltyInfo(req: Request, res: Response) {
    try {
      const { collection } = req.params;

      const royaltyInfo = await this.nftService.getRoyaltyInfo(collection);

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

  async claimRoyalties(req: Request, res: Response) {
    try {
      const { collection, recipient } = req.body;

      const txHash = await this.nftService.claimRoyalties({
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
}