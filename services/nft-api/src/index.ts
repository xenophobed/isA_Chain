/**
 * NFT Service - HTTP API Adapter for isA_Chain NFT Smart Contracts
 * 
 * This service provides RESTful APIs for NFT operations:
 * - ISANFT: NFT minting and management
 * - NFTMarketplace: NFT trading and marketplace operations
 */

import express from 'express';
import cors from 'cors';
import helmet from 'helmet';
import multer from 'multer';
import rateLimit from 'express-rate-limit';
import { NFTController } from './controllers/nft.controller';
import { MarketplaceController } from './controllers/marketplace.controller';
import { blockchainService } from './services/blockchain.service';
import { ipfsService } from './services/ipfs.service';
import { consulService } from './services/consul.service';
import { logger } from './utils/logger';
import { config } from './config';
import { errorHandler } from './middleware/error.middleware';
import { validateRequest } from './middleware/validation.middleware';

const app = express();
const PORT = process.env.PORT || 8312;

// Configure multer for file uploads
const upload = multer({
  storage: multer.memoryStorage(),
  limits: {
    fileSize: 10 * 1024 * 1024, // 10MB limit
  },
  fileFilter: (req, file, cb) => {
    // Accept images only
    if (!file.mimetype.startsWith('image/')) {
      return cb(new Error('Only image files are allowed'));
    }
    cb(null, true);
  }
});

// Middleware
app.use(helmet());
app.use(cors());
app.use(express.json());
app.use(express.urlencoded({ extended: true }));

// Rate limiting
const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 100, // limit each IP to 100 requests per windowMs
  message: 'Too many requests from this IP'
});
app.use('/api/', limiter);

// Health check
app.get('/health', (req, res) => {
  res.json({
    status: 'healthy',
    service: 'nft-service',
    version: '1.0.0',
    timestamp: new Date().toISOString()
  });
});

// Initialize services
async function initializeServices() {
  try {
    // Services are already initialized as singletons
    logger.info('Connected to blockchain');
    logger.info('Connected to IPFS');

    // Initialize controllers
    const nftController = new NFTController(blockchainService, ipfsService);
    const marketplaceController = new MarketplaceController(blockchainService);

    // API Routes
    const apiRouter = express.Router();

    // ===== NFT Management Routes =====
    
    // Collection endpoints
    apiRouter.get('/collections', nftController.getCollections.bind(nftController));
    apiRouter.post('/collections/create', validateRequest('createCollection'), nftController.createCollection.bind(nftController));
    apiRouter.get('/collections/:address', nftController.getCollectionDetails.bind(nftController));
    apiRouter.get('/collections/:address/stats', nftController.getCollectionStats.bind(nftController));
    
    // Minting endpoints
    apiRouter.post('/mint', upload.single('image'), validateRequest('mintNFT'), nftController.mintNFT.bind(nftController));
    apiRouter.post('/mint/batch', upload.array('images', 10), validateRequest('batchMint'), nftController.batchMint.bind(nftController));
    apiRouter.post('/mint/lazy', validateRequest('lazyMint'), nftController.lazyMint.bind(nftController));
    
    // NFT query endpoints
    apiRouter.get('/tokens/:tokenId', nftController.getTokenDetails.bind(nftController));
    apiRouter.get('/tokens/:tokenId/metadata', nftController.getTokenMetadata.bind(nftController));
    apiRouter.get('/tokens/:tokenId/history', nftController.getTokenHistory.bind(nftController));
    apiRouter.get('/owner/:address/tokens', nftController.getTokensByOwner.bind(nftController));
    
    // NFT management endpoints
    apiRouter.post('/transfer', validateRequest('transferNFT'), nftController.transferNFT.bind(nftController));
    apiRouter.post('/burn', validateRequest('burnNFT'), nftController.burnNFT.bind(nftController));
    apiRouter.post('/approve', validateRequest('approveNFT'), nftController.approveNFT.bind(nftController));
    apiRouter.post('/metadata/update', validateRequest('updateMetadata'), nftController.updateMetadata.bind(nftController));
    
    // ===== Marketplace Routes =====
    
    // Listing endpoints
    apiRouter.get('/marketplace/listings', marketplaceController.getListings.bind(marketplaceController));
    apiRouter.get('/marketplace/listings/:listingId', marketplaceController.getListingDetails.bind(marketplaceController));
    apiRouter.post('/marketplace/list', validateRequest('createListing'), marketplaceController.createListing.bind(marketplaceController));
    apiRouter.post('/marketplace/delist', validateRequest('cancelListing'), marketplaceController.cancelListing.bind(marketplaceController));
    apiRouter.put('/marketplace/price', validateRequest('updatePrice'), marketplaceController.updatePrice.bind(marketplaceController));
    
    // Trading endpoints
    apiRouter.post('/marketplace/buy', validateRequest('buyNFT'), marketplaceController.buyNFT.bind(marketplaceController));
    apiRouter.post('/marketplace/offer', validateRequest('makeOffer'), marketplaceController.makeOffer.bind(marketplaceController));
    apiRouter.post('/marketplace/offer/accept', validateRequest('acceptOffer'), marketplaceController.acceptOffer.bind(marketplaceController));
    apiRouter.post('/marketplace/offer/reject', validateRequest('rejectOffer'), marketplaceController.rejectOffer.bind(marketplaceController));
    apiRouter.get('/marketplace/offers/:tokenId', marketplaceController.getOffers.bind(marketplaceController));
    
    // Auction endpoints
    apiRouter.post('/marketplace/auction/create', validateRequest('createAuction'), marketplaceController.createAuction.bind(marketplaceController));
    apiRouter.post('/marketplace/auction/bid', validateRequest('placeBid'), marketplaceController.placeBid.bind(marketplaceController));
    apiRouter.post('/marketplace/auction/end', validateRequest('endAuction'), marketplaceController.endAuction.bind(marketplaceController));
    apiRouter.get('/marketplace/auctions', marketplaceController.getActiveAuctions.bind(marketplaceController));
    
    // Analytics endpoints
    apiRouter.get('/analytics/floor-price/:collection', marketplaceController.getFloorPrice.bind(marketplaceController));
    apiRouter.get('/analytics/volume/:collection', marketplaceController.getTradingVolume.bind(marketplaceController));
    apiRouter.get('/analytics/trending', marketplaceController.getTrendingCollections.bind(marketplaceController));
    apiRouter.get('/analytics/activity', marketplaceController.getRecentActivity.bind(marketplaceController));
    
    // Royalty endpoints
    apiRouter.post('/royalty/set', validateRequest('setRoyalty'), nftController.setRoyalty.bind(nftController));
    apiRouter.get('/royalty/:collection', nftController.getRoyaltyInfo.bind(nftController));
    apiRouter.post('/royalty/claim', validateRequest('claimRoyalties'), nftController.claimRoyalties.bind(nftController));

    // Mount API routes
    app.use('/api/v1', apiRouter);

    // Error handling middleware (must be last)
    app.use(errorHandler);

    // Register with Consul
    await consulService.register();
    logger.info('Registered with Consul');

    // Start server
    app.listen(PORT, () => {
      logger.info(`NFT service running on port ${PORT}`);
      console.log(`
        🎨 NFT Service Started
        ========================
        Port: ${PORT}
        Environment: ${process.env.NODE_ENV || 'development'}
        IPFS: ${config.ipfs.gateway}
        
        Available endpoints:
        - POST /api/v1/mint
        - GET  /api/v1/collections
        - GET  /api/v1/marketplace/listings
        - POST /api/v1/marketplace/buy
        ... and more
        
        Access via Gateway: http://localhost:8000/api/v1/nft-service/
      `);
    });

  } catch (error) {
    logger.error('Failed to initialize services:', error);
    process.exit(1);
  }
}

// Graceful shutdown
process.on('SIGTERM', async () => {
  logger.info('SIGTERM received, shutting down gracefully');
  await consulService.deregister();
  process.exit(0);
});

process.on('unhandledRejection', (error) => {
  logger.error('Unhandled rejection:', error);
  process.exit(1);
});

// Start the service
initializeServices();