/**
 * Tools Service - Developer Tools for Smart Contract Interactions
 * 
 * This service provides generic tools for blockchain developers:
 * - Generic contract calls (read/write)
 * - ABI encoding/decoding
 * - Transaction debugging
 * - Gas estimation
 * - Event filtering
 * - Contract verification
 */

import express from 'express';
import cors from 'cors';
import helmet from 'helmet';
import rateLimit from 'express-rate-limit';
import { ToolsController } from './controllers/tools.controller';
import { BlockchainService } from './services/blockchain.service';
import { ABIService } from './services/abi.service';
import { CacheService } from './services/cache.service';
import { ConsulService } from './services/consul.service';
import { logger } from './utils/logger';
import { config } from './config';
import { errorHandler } from './middleware/error.middleware';
import { validateRequest } from './middleware/validation.middleware';

const app = express();
const PORT = process.env.PORT || 8315;

// Middleware
app.use(helmet());
app.use(cors());
app.use(express.json({ limit: '10mb' })); // Larger limit for ABI uploads
app.use(express.urlencoded({ extended: true }));

// Rate limiting - more lenient for developer tools
const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 500, // 500 requests per window for dev tools
  message: 'Too many requests from this IP'
});
app.use('/api/', limiter);

// Health check
app.get('/health', (req, res) => {
  res.json({
    status: 'healthy',
    service: 'tools-service',
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

    // Initialize cache
    const cacheService = await CacheService.getInstance();
    logger.info('Cache service initialized');

    // Initialize ABI service
    const abiService = new ABIService(cacheService);

    // Initialize controller
    const toolsController = new ToolsController(
      blockchainService,
      abiService,
      cacheService
    );

    // API Routes
    const apiRouter = express.Router();

    // ===== Contract Interaction Routes =====
    
    // Generic contract calls
    apiRouter.post('/contract/call', 
      validateRequest('contractCall'), 
      toolsController.callContract.bind(toolsController)
    );
    
    apiRouter.post('/contract/read', 
      validateRequest('contractRead'), 
      toolsController.readContract.bind(toolsController)
    );
    
    apiRouter.post('/contract/write', 
      validateRequest('contractWrite'), 
      toolsController.writeContract.bind(toolsController)
    );
    
    apiRouter.post('/contract/deploy', 
      validateRequest('contractDeploy'), 
      toolsController.deployContract.bind(toolsController)
    );

    // ===== ABI Management Routes =====
    
    apiRouter.post('/abi/register', 
      validateRequest('registerABI'), 
      toolsController.registerABI.bind(toolsController)
    );
    
    apiRouter.get('/abi/:contractAddress', 
      toolsController.getABI.bind(toolsController)
    );
    
    apiRouter.post('/abi/encode', 
      validateRequest('encodeFunction'), 
      toolsController.encodeFunction.bind(toolsController)
    );
    
    apiRouter.post('/abi/decode', 
      validateRequest('decodeFunction'), 
      toolsController.decodeFunction.bind(toolsController)
    );
    
    apiRouter.post('/abi/decode-logs', 
      validateRequest('decodeLogs'), 
      toolsController.decodeLogs.bind(toolsController)
    );

    // ===== Transaction Tools Routes =====
    
    apiRouter.post('/transaction/estimate-gas', 
      validateRequest('estimateGas'), 
      toolsController.estimateGas.bind(toolsController)
    );
    
    apiRouter.post('/transaction/simulate', 
      validateRequest('simulateTransaction'), 
      toolsController.simulateTransaction.bind(toolsController)
    );
    
    apiRouter.get('/transaction/trace/:hash', 
      toolsController.traceTransaction.bind(toolsController)
    );
    
    apiRouter.post('/transaction/send-raw', 
      validateRequest('sendRawTransaction'), 
      toolsController.sendRawTransaction.bind(toolsController)
    );
    
    apiRouter.get('/transaction/receipt/:hash', 
      toolsController.getTransactionReceipt.bind(toolsController)
    );

    // ===== Event Filtering Routes =====
    
    apiRouter.post('/events/filter', 
      validateRequest('filterEvents'), 
      toolsController.filterEvents.bind(toolsController)
    );
    
    apiRouter.post('/events/subscribe', 
      validateRequest('subscribeEvents'), 
      toolsController.subscribeToEvents.bind(toolsController)
    );
    
    apiRouter.delete('/events/unsubscribe/:subscriptionId', 
      toolsController.unsubscribeFromEvents.bind(toolsController)
    );

    // ===== Blockchain Query Routes =====
    
    apiRouter.get('/block/:blockNumber', 
      toolsController.getBlock.bind(toolsController)
    );
    
    apiRouter.get('/account/:address', 
      toolsController.getAccountInfo.bind(toolsController)
    );
    
    apiRouter.get('/code/:address', 
      toolsController.getContractCode.bind(toolsController)
    );
    
    apiRouter.get('/storage/:address/:slot', 
      toolsController.getStorageAt.bind(toolsController)
    );
    
    apiRouter.get('/nonce/:address', 
      toolsController.getNonce.bind(toolsController)
    );
    
    apiRouter.get('/gas-price', 
      toolsController.getGasPrice.bind(toolsController)
    );

    // ===== Signature Tools Routes =====
    
    apiRouter.post('/signature/sign', 
      validateRequest('signMessage'), 
      toolsController.signMessage.bind(toolsController)
    );
    
    apiRouter.post('/signature/verify', 
      validateRequest('verifySignature'), 
      toolsController.verifySignature.bind(toolsController)
    );
    
    apiRouter.post('/signature/recover', 
      validateRequest('recoverAddress'), 
      toolsController.recoverAddress.bind(toolsController)
    );

    // ===== Utility Routes =====
    
    apiRouter.post('/utils/keccak256', 
      validateRequest('keccak256'), 
      toolsController.keccak256.bind(toolsController)
    );
    
    apiRouter.post('/utils/encode-packed', 
      validateRequest('encodePacked'), 
      toolsController.encodePacked.bind(toolsController)
    );
    
    apiRouter.post('/utils/to-hex', 
      validateRequest('toHex'), 
      toolsController.toHex.bind(toolsController)
    );
    
    apiRouter.post('/utils/from-hex', 
      validateRequest('fromHex'), 
      toolsController.fromHex.bind(toolsController)
    );
    
    apiRouter.post('/utils/checksum-address', 
      validateRequest('checksumAddress'), 
      toolsController.checksumAddress.bind(toolsController)
    );

    // ===== Contract Verification Routes =====
    
    apiRouter.post('/verify/contract', 
      validateRequest('verifyContract'), 
      toolsController.verifyContract.bind(toolsController)
    );
    
    apiRouter.get('/verify/status/:guid', 
      toolsController.getVerificationStatus.bind(toolsController)
    );

    // ===== Debugging Routes =====
    
    apiRouter.post('/debug/call-trace', 
      validateRequest('debugCallTrace'), 
      toolsController.debugCallTrace.bind(toolsController)
    );
    
    apiRouter.post('/debug/storage-diff', 
      validateRequest('debugStorageDiff'), 
      toolsController.debugStorageDiff.bind(toolsController)
    );
    
    apiRouter.post('/debug/revert-reason', 
      validateRequest('getRevertReason'), 
      toolsController.getRevertReason.bind(toolsController)
    );

    // ===== Batch Operations Routes =====
    
    apiRouter.post('/batch/call', 
      validateRequest('batchCall'), 
      toolsController.batchCall.bind(toolsController)
    );
    
    apiRouter.post('/batch/transaction', 
      validateRequest('batchTransaction'), 
      toolsController.batchTransaction.bind(toolsController)
    );

    // Mount API routes
    app.use('/api/v1', apiRouter);

    // Error handling middleware
    app.use(errorHandler);

    // Register with Consul
    const consulService = new ConsulService(config.consul);
    await consulService.register({
      name: 'tools-service',
      port: PORT,
      tags: ['blockchain', 'developer', 'tools', 'http'],
      check: {
        http: `http://localhost:${PORT}/health`,
        interval: '10s',
        timeout: '5s'
      }
    });
    logger.info('Registered with Consul');

    // Start server
    const server = app.listen(PORT, () => {
      logger.info(`Tools service running on port ${PORT}`);
      console.log(`
        🔧 Tools Service Started
        ========================
        Port: ${PORT}
        Environment: ${process.env.NODE_ENV || 'development'}
        Cache: ${config.cache.enabled ? 'Enabled' : 'Disabled'}
        
        Available tools:
        - Generic contract calls
        - ABI encoding/decoding
        - Transaction debugging
        - Event filtering
        - Gas estimation
        - Signature verification
        ... and more
        
        Access via Gateway: http://localhost:8000/api/v1/tools-service/
        
        Example usage:
        
        # Read contract state
        curl -X POST http://localhost:8000/api/v1/tools-service/contract/read \\
          -H "Content-Type: application/json" \\
          -d '{
            "contract": "0x...",
            "method": "balanceOf",
            "params": ["0x..."]
          }'
        
        # Encode function call
        curl -X POST http://localhost:8000/api/v1/tools-service/abi/encode \\
          -H "Content-Type: application/json" \\
          -d '{
            "abi": [...],
            "functionName": "transfer",
            "params": ["0x...", "1000000"]
          }'
      `);
    });

    // WebSocket support for event subscriptions
    const WebSocket = require('ws');
    const wss = new WebSocket.Server({ server });
    
    wss.on('connection', (ws) => {
      logger.info('WebSocket client connected');
      
      ws.on('message', async (message) => {
        try {
          const data = JSON.parse(message.toString());
          if (data.type === 'subscribe') {
            await toolsController.handleWebSocketSubscription(ws, data);
          }
        } catch (error) {
          ws.send(JSON.stringify({ error: 'Invalid message format' }));
        }
      });
      
      ws.on('close', () => {
        logger.info('WebSocket client disconnected');
      });
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
  await consulService.deregister('tools-service');
  process.exit(0);
});

process.on('unhandledRejection', (error) => {
  logger.error('Unhandled rejection:', error);
  process.exit(1);
});

// Start the service
initializeServices();