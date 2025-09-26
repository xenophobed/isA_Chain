/**
 * DeFi Service - HTTP API Adapter for isA_Chain DeFi Smart Contracts
 * 
 * This service provides RESTful APIs for interacting with DeFi contracts:
 * - SimpleDEX: Token swaps and liquidity
 * - StakingPool: Token staking
 * - YieldFarming: Yield farming operations
 * - LendingProtocol: Lending and borrowing
 */

import express from 'express';
import { ethers } from 'ethers';
import cors from 'cors';
import helmet from 'helmet';
import rateLimit from 'express-rate-limit';
import { DeFiController } from './controllers/defi.controller';
import { BlockchainService } from './services/blockchain.service';
import { ConsulService } from './services/consul.service';
import { logger } from './utils/logger';
import { config } from './config';

const app = express();
const PORT = process.env.PORT || 8311;

// Middleware
app.use(helmet());
app.use(cors());
app.use(express.json());

// Rate limiting
const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 100 // limit each IP to 100 requests per windowMs
});
app.use('/api/', limiter);

// Health check
app.get('/health', (req, res) => {
  res.json({ 
    status: 'healthy', 
    service: 'defi-service',
    version: '1.0.0',
    timestamp: new Date().toISOString()
  });
});

// Initialize services
async function initializeServices() {
  try {
    // Connect to blockchain
    const blockchainService = await BlockchainService.getInstance();
    logger.info('Connected to blockchain');

    // Initialize controller
    const defiController = new DeFiController(blockchainService);

    // DeFi API Routes
    const apiRouter = express.Router();

    // Swap endpoints
    apiRouter.get('/pools', defiController.getPools.bind(defiController));
    apiRouter.post('/swap/quote', defiController.getSwapQuote.bind(defiController));
    apiRouter.post('/swap/execute', defiController.executeSwap.bind(defiController));
    
    // Liquidity endpoints
    apiRouter.post('/liquidity/add', defiController.addLiquidity.bind(defiController));
    apiRouter.post('/liquidity/remove', defiController.removeLiquidity.bind(defiController));
    apiRouter.get('/liquidity/positions/:address', defiController.getLiquidityPositions.bind(defiController));
    
    // Staking endpoints
    apiRouter.post('/stake', defiController.stake.bind(defiController));
    apiRouter.post('/unstake', defiController.unstake.bind(defiController));
    apiRouter.get('/stake/rewards/:address', defiController.getStakingRewards.bind(defiController));
    apiRouter.post('/stake/claim', defiController.claimRewards.bind(defiController));
    
    // Yield farming endpoints
    apiRouter.get('/farms', defiController.getFarms.bind(defiController));
    apiRouter.post('/farm/deposit', defiController.depositToFarm.bind(defiController));
    apiRouter.post('/farm/withdraw', defiController.withdrawFromFarm.bind(defiController));
    apiRouter.post('/farm/harvest', defiController.harvestYield.bind(defiController));
    
    // Lending endpoints
    apiRouter.get('/lending/markets', defiController.getLendingMarkets.bind(defiController));
    apiRouter.post('/lending/supply', defiController.supplyAsset.bind(defiController));
    apiRouter.post('/lending/borrow', defiController.borrowAsset.bind(defiController));
    apiRouter.post('/lending/repay', defiController.repayLoan.bind(defiController));
    apiRouter.get('/lending/position/:address', defiController.getLendingPosition.bind(defiController));

    // Mount API routes
    app.use('/api/v1', apiRouter);

    // Register with Consul
    const consulService = new ConsulService(config.consul);
    await consulService.register({
      name: 'defi-service',
      port: PORT,
      tags: ['blockchain', 'defi', 'http'],
      check: {
        http: `http://localhost:${PORT}/health`,
        interval: '10s'
      }
    });
    logger.info('Registered with Consul');

    // Start server
    app.listen(PORT, () => {
      logger.info(`DeFi service running on port ${PORT}`);
      console.log(`
        🏦 DeFi Service Started
        ========================
        Port: ${PORT}
        Environment: ${process.env.NODE_ENV || 'development'}
        
        Available endpoints:
        - GET  /api/v1/pools
        - POST /api/v1/swap/execute
        - POST /api/v1/stake
        - GET  /api/v1/farms
        ... and more
        
        Access via Gateway: http://localhost:8000/api/v1/defi-service/
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
  const consulService = new ConsulService(config.consul);
  await consulService.deregister('defi-service');
  process.exit(0);
});

// Start the service
initializeServices();