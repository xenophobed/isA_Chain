import { Request, Response } from 'express';
import { BlockchainService } from '../services/blockchain.service';
import { IPFSService } from '../services/ipfs.service';
import { IsaNFTService } from '../services/contracts/isaNFT.service';
import * as collectionHandler from './nft/collection.handler';
import * as mintingHandler from './nft/minting.handler';
import * as queryHandler from './nft/query.handler';
import * as lifecycleHandler from './nft/lifecycle.handler';
import * as royaltyHandler from './nft/royalty.handler';

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
    return collectionHandler.getCollections(this.nftService, req, res);
  }

  async createCollection(req: Request, res: Response) {
    return collectionHandler.createCollection(this.nftService, req, res);
  }

  async getCollectionDetails(req: Request, res: Response) {
    return collectionHandler.getCollectionDetails(this.nftService, req, res);
  }

  async getCollectionStats(req: Request, res: Response) {
    return collectionHandler.getCollectionStats(this.nftService, req, res);
  }

  // ============ NFT Minting ============

  async mintNFT(req: Request, res: Response) {
    return mintingHandler.mintNFT(this.nftService, this.ipfs, req, res);
  }

  async batchMint(req: Request, res: Response) {
    return mintingHandler.batchMint(this.nftService, this.ipfs, req, res);
  }

  async lazyMint(req: Request, res: Response) {
    return mintingHandler.lazyMint(this.nftService, req, res);
  }

  // ============ NFT Queries ============

  async getTokenDetails(req: Request, res: Response) {
    return queryHandler.getTokenDetails(this.nftService, req, res);
  }

  async getTokenMetadata(req: Request, res: Response) {
    return queryHandler.getTokenMetadata(this.nftService, this.ipfs, req, res);
  }

  async getTokenHistory(req: Request, res: Response) {
    return queryHandler.getTokenHistory(this.nftService, req, res);
  }

  async getTokensByOwner(req: Request, res: Response) {
    return queryHandler.getTokensByOwner(this.nftService, req, res);
  }

  // ============ NFT Management ============

  async transferNFT(req: Request, res: Response) {
    return lifecycleHandler.transferNFT(this.nftService, req, res);
  }

  async burnNFT(req: Request, res: Response) {
    return lifecycleHandler.burnNFT(this.nftService, req, res);
  }

  async approveNFT(req: Request, res: Response) {
    return lifecycleHandler.approveNFT(this.nftService, req, res);
  }

  async updateMetadata(req: Request, res: Response) {
    return lifecycleHandler.updateMetadata(this.nftService, this.ipfs, req, res);
  }

  // ============ Royalty Management ============

  async setRoyalty(req: Request, res: Response) {
    return royaltyHandler.setRoyalty(this.nftService, req, res);
  }

  async getRoyaltyInfo(req: Request, res: Response) {
    return royaltyHandler.getRoyaltyInfo(this.nftService, req, res);
  }

  async claimRoyalties(req: Request, res: Response) {
    return royaltyHandler.claimRoyalties(this.nftService, req, res);
  }
}