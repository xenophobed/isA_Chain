/**
 * @fileoverview MCP Bridge for isA_Chain - Agent/Model/MCP Integration Layer
 * 
 * This module provides the bridge between the existing Agent/Model/MCP stack
 * and the isA_Chain blockchain ecosystem, enabling AI-native blockchain operations.
 */

import { Server } from '@modelcontextprotocol/sdk/server/index.js';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
  ListResourcesRequestSchema,
  ReadResourceRequestSchema,
  Tool,
  Resource,
} from '@modelcontextprotocol/sdk/types.js';
import { ethers } from 'ethers';
import { BlockchainService } from './services/blockchain.js';
import { ContractService } from './services/contracts.js';
import { WalletService } from './services/wallet.js';
import { DeFiService } from './services/defi.js';
import { NFTService } from './services/nft.js';
import { AnalyticsService } from './services/analytics.js';

/**
 * MCP Bridge Server for isA_Chain
 * Provides AI agents with blockchain capabilities through MCP protocol
 */
class IsAChainMCPServer {
  private server: Server;
  private blockchainService: BlockchainService;
  private contractService: ContractService;
  private walletService: WalletService;
  private defiService: DeFiService;
  private nftService: NFTService;
  private analyticsService: AnalyticsService;

  constructor() {
    this.server = new Server(
      {
        name: 'isa-chain-mcp-bridge',
        version: '1.0.0',
      },
      {
        capabilities: {
          tools: {},
          resources: {},
        },
      }
    );

    this.initializeServices();
    this.setupHandlers();
  }

  /**
   * Initialize blockchain services
   */
  private async initializeServices(): Promise<void> {
    // Initialize services with environment configuration
    const rpcUrl = process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545';
    const privateKey = process.env.PRIVATE_KEY;
    
    if (!privateKey) {
      throw new Error('PRIVATE_KEY environment variable is required');
    }

    const provider = new ethers.JsonRpcProvider(rpcUrl);
    const wallet = new ethers.Wallet(privateKey, provider);

    this.blockchainService = new BlockchainService(provider);
    this.contractService = new ContractService(wallet);
    this.walletService = new WalletService(wallet);
    this.defiService = new DeFiService(wallet);
    this.nftService = new NFTService(wallet);
    this.analyticsService = new AnalyticsService(provider);
  }

  /**
   * Setup MCP handlers
   */
  private setupHandlers(): void {
    this.server.setRequestHandler(ListToolsRequestSchema, async () => ({
      tools: this.getTools(),
    }));

    this.server.setRequestHandler(CallToolRequestSchema, async (request) => {
      const { name, arguments: args } = request.params;
      return await this.handleToolCall(name, args || {});
    });

    this.server.setRequestHandler(ListResourcesRequestSchema, async () => ({
      resources: this.getResources(),
    }));

    this.server.setRequestHandler(ReadResourceRequestSchema, async (request) => {
      const { uri } = request.params;
      return await this.handleResourceRead(uri);
    });
  }

  /**
   * Define available tools for AI agents
   */
  private getTools(): Tool[] {
    return [
      // Wallet Operations
      {
        name: 'get_wallet_balance',
        description: 'Get wallet balance for a specific token',
        inputSchema: {
          type: 'object',
          properties: {
            tokenAddress: { type: 'string', description: 'Token contract address (optional, defaults to native token)' },
            walletAddress: { type: 'string', description: 'Wallet address (optional, uses default wallet)' },
          },
        },
      },
      {
        name: 'send_transaction',
        description: 'Send tokens to another address',
        inputSchema: {
          type: 'object',
          required: ['to', 'amount'],
          properties: {
            to: { type: 'string', description: 'Recipient address' },
            amount: { type: 'string', description: 'Amount to send' },
            tokenAddress: { type: 'string', description: 'Token contract address (optional)' },
          },
        },
      },
      
      // DeFi Operations
      {
        name: 'swap_tokens',
        description: 'Swap tokens on DEX',
        inputSchema: {
          type: 'object',
          required: ['tokenIn', 'tokenOut', 'amountIn'],
          properties: {
            tokenIn: { type: 'string', description: 'Input token address' },
            tokenOut: { type: 'string', description: 'Output token address' },
            amountIn: { type: 'string', description: 'Amount of input tokens' },
            minAmountOut: { type: 'string', description: 'Minimum output amount' },
            slippage: { type: 'number', description: 'Slippage tolerance (default 0.5%)' },
          },
        },
      },
      {
        name: 'add_liquidity',
        description: 'Add liquidity to a DEX pool',
        inputSchema: {
          type: 'object',
          required: ['tokenA', 'tokenB', 'amountA', 'amountB'],
          properties: {
            tokenA: { type: 'string', description: 'First token address' },
            tokenB: { type: 'string', description: 'Second token address' },
            amountA: { type: 'string', description: 'Amount of first token' },
            amountB: { type: 'string', description: 'Amount of second token' },
          },
        },
      },
      {
        name: 'stake_tokens',
        description: 'Stake tokens in a staking pool',
        inputSchema: {
          type: 'object',
          required: ['poolId', 'amount'],
          properties: {
            poolId: { type: 'number', description: 'Staking pool ID' },
            amount: { type: 'string', description: 'Amount to stake' },
            lockDuration: { type: 'number', description: 'Lock duration in seconds' },
          },
        },
      },
      {
        name: 'lend_tokens',
        description: 'Lend tokens to earn interest',
        inputSchema: {
          type: 'object',
          required: ['asset', 'amount'],
          properties: {
            asset: { type: 'string', description: 'Asset to lend' },
            amount: { type: 'string', description: 'Amount to lend' },
          },
        },
      },
      {
        name: 'borrow_tokens',
        description: 'Borrow tokens using collateral',
        inputSchema: {
          type: 'object',
          required: ['asset', 'amount'],
          properties: {
            asset: { type: 'string', description: 'Asset to borrow' },
            amount: { type: 'string', description: 'Amount to borrow' },
            collateralAsset: { type: 'string', description: 'Collateral asset' },
          },
        },
      },

      // NFT Operations
      {
        name: 'mint_nft',
        description: 'Mint a new NFT',
        inputSchema: {
          type: 'object',
          required: ['to', 'tokenURI'],
          properties: {
            to: { type: 'string', description: 'Recipient address' },
            tokenURI: { type: 'string', description: 'Metadata URI' },
            quantity: { type: 'number', description: 'Number of NFTs to mint (default 1)' },
          },
        },
      },
      {
        name: 'list_nft_for_sale',
        description: 'List NFT for sale on marketplace',
        inputSchema: {
          type: 'object',
          required: ['tokenId', 'price'],
          properties: {
            tokenId: { type: 'string', description: 'NFT token ID' },
            price: { type: 'string', description: 'Sale price' },
            paymentToken: { type: 'string', description: 'Payment token address' },
            duration: { type: 'number', description: 'Sale duration in seconds' },
          },
        },
      },
      {
        name: 'buy_nft',
        description: 'Purchase an NFT from marketplace',
        inputSchema: {
          type: 'object',
          required: ['tokenId'],
          properties: {
            tokenId: { type: 'string', description: 'NFT token ID to purchase' },
            maxPrice: { type: 'string', description: 'Maximum price willing to pay' },
          },
        },
      },

      // Analytics and Monitoring
      {
        name: 'get_portfolio_value',
        description: 'Get total portfolio value across all protocols',
        inputSchema: {
          type: 'object',
          properties: {
            walletAddress: { type: 'string', description: 'Wallet address (optional)' },
            includeStaking: { type: 'boolean', description: 'Include staking positions' },
            includeLending: { type: 'boolean', description: 'Include lending positions' },
            includeNFTs: { type: 'boolean', description: 'Include NFT valuations' },
          },
        },
      },
      {
        name: 'get_yield_opportunities',
        description: 'Find best yield opportunities across protocols',
        inputSchema: {
          type: 'object',
          properties: {
            asset: { type: 'string', description: 'Asset to find yields for' },
            amount: { type: 'string', description: 'Amount to invest' },
            riskLevel: { type: 'string', enum: ['low', 'medium', 'high'], description: 'Risk tolerance' },
            minAPY: { type: 'number', description: 'Minimum APY requirement' },
          },
        },
      },
      {
        name: 'analyze_transaction',
        description: 'Analyze transaction details and impact',
        inputSchema: {
          type: 'object',
          required: ['txHash'],
          properties: {
            txHash: { type: 'string', description: 'Transaction hash' },
            includeGasAnalysis: { type: 'boolean', description: 'Include gas analysis' },
            includePriceImpact: { type: 'boolean', description: 'Include price impact analysis' },
          },
        },
      },

      // Advanced Trading
      {
        name: 'place_limit_order',
        description: 'Place a limit order on the exchange',
        inputSchema: {
          type: 'object',
          required: ['pair', 'side', 'amount', 'price'],
          properties: {
            pair: { type: 'string', description: 'Trading pair (e.g., "ETH/USDC")' },
            side: { type: 'string', enum: ['buy', 'sell'], description: 'Order side' },
            amount: { type: 'string', description: 'Order amount' },
            price: { type: 'string', description: 'Limit price' },
            timeInForce: { type: 'string', enum: ['GTC', 'GTT', 'IOC', 'FOK'], description: 'Time in force' },
          },
        },
      },
      {
        name: 'set_stop_loss',
        description: 'Set a stop-loss order',
        inputSchema: {
          type: 'object',
          required: ['pair', 'side', 'amount', 'stopPrice'],
          properties: {
            pair: { type: 'string', description: 'Trading pair' },
            side: { type: 'string', enum: ['buy', 'sell'], description: 'Order side' },
            amount: { type: 'string', description: 'Order amount' },
            stopPrice: { type: 'string', description: 'Stop trigger price' },
            limitPrice: { type: 'string', description: 'Limit price after trigger' },
          },
        },
      },

      // Privacy Operations
      {
        name: 'private_transfer',
        description: 'Make a private transfer using privacy pool',
        inputSchema: {
          type: 'object',
          required: ['amount', 'denomination'],
          properties: {
            amount: { type: 'string', description: 'Amount to transfer privately' },
            denomination: { type: 'string', description: 'Privacy pool denomination' },
            recipient: { type: 'string', description: 'Recipient commitment hash' },
          },
        },
      },
    ];
  }

  /**
   * Define available resources for AI agents
   */
  private getResources(): Resource[] {
    return [
      {
        uri: 'isa://blockchain/status',
        name: 'Blockchain Status',
        description: 'Current blockchain network status and metrics',
        mimeType: 'application/json',
      },
      {
        uri: 'isa://defi/pools',
        name: 'DeFi Pools',
        description: 'Available DeFi pools and their APYs',
        mimeType: 'application/json',
      },
      {
        uri: 'isa://nft/collections',
        name: 'NFT Collections',
        description: 'Available NFT collections and metadata',
        mimeType: 'application/json',
      },
      {
        uri: 'isa://exchange/orderbook',
        name: 'Exchange Order Book',
        description: 'Current order book state for trading pairs',
        mimeType: 'application/json',
      },
      {
        uri: 'isa://analytics/dashboard',
        name: 'Analytics Dashboard',
        description: 'Real-time analytics and metrics dashboard',
        mimeType: 'application/json',
      },
    ];
  }

  /**
   * Handle tool calls from AI agents
   */
  private async handleToolCall(name: string, args: any): Promise<any> {
    try {
      switch (name) {
        // Wallet operations
        case 'get_wallet_balance':
          return await this.walletService.getBalance(args.tokenAddress, args.walletAddress);
        
        case 'send_transaction':
          return await this.walletService.sendTransaction(args.to, args.amount, args.tokenAddress);

        // DeFi operations
        case 'swap_tokens':
          return await this.defiService.swapTokens(
            args.tokenIn,
            args.tokenOut,
            args.amountIn,
            args.minAmountOut,
            args.slippage
          );

        case 'add_liquidity':
          return await this.defiService.addLiquidity(
            args.tokenA,
            args.tokenB,
            args.amountA,
            args.amountB
          );

        case 'stake_tokens':
          return await this.defiService.stakeTokens(args.poolId, args.amount, args.lockDuration);

        case 'lend_tokens':
          return await this.defiService.lendTokens(args.asset, args.amount);

        case 'borrow_tokens':
          return await this.defiService.borrowTokens(args.asset, args.amount, args.collateralAsset);

        // NFT operations
        case 'mint_nft':
          return await this.nftService.mintNFT(args.to, args.tokenURI, args.quantity);

        case 'list_nft_for_sale':
          return await this.nftService.listForSale(
            args.tokenId,
            args.price,
            args.paymentToken,
            args.duration
          );

        case 'buy_nft':
          return await this.nftService.buyNFT(args.tokenId, args.maxPrice);

        // Analytics
        case 'get_portfolio_value':
          return await this.analyticsService.getPortfolioValue(
            args.walletAddress,
            args.includeStaking,
            args.includeLending,
            args.includeNFTs
          );

        case 'get_yield_opportunities':
          return await this.analyticsService.getYieldOpportunities(
            args.asset,
            args.amount,
            args.riskLevel,
            args.minAPY
          );

        case 'analyze_transaction':
          return await this.analyticsService.analyzeTransaction(
            args.txHash,
            args.includeGasAnalysis,
            args.includePriceImpact
          );

        // Trading
        case 'place_limit_order':
          return await this.defiService.placeLimitOrder(
            args.pair,
            args.side,
            args.amount,
            args.price,
            args.timeInForce
          );

        case 'set_stop_loss':
          return await this.defiService.setStopLoss(
            args.pair,
            args.side,
            args.amount,
            args.stopPrice,
            args.limitPrice
          );

        // Privacy
        case 'private_transfer':
          return await this.defiService.privateTransfer(
            args.amount,
            args.denomination,
            args.recipient
          );

        default:
          throw new Error(`Unknown tool: ${name}`);
      }
    } catch (error) {
      console.error(`Error executing tool ${name}:`, error);
      throw error;
    }
  }

  /**
   * Handle resource reads
   */
  private async handleResourceRead(uri: string): Promise<any> {
    try {
      const [, , resource] = uri.split('/');

      switch (resource) {
        case 'status':
          return {
            contents: [{
              uri,
              mimeType: 'application/json',
              text: JSON.stringify(await this.blockchainService.getStatus()),
            }],
          };

        case 'pools':
          return {
            contents: [{
              uri,
              mimeType: 'application/json',
              text: JSON.stringify(await this.defiService.getAllPools()),
            }],
          };

        case 'collections':
          return {
            contents: [{
              uri,
              mimeType: 'application/json',
              text: JSON.stringify(await this.nftService.getAllCollections()),
            }],
          };

        case 'orderbook':
          return {
            contents: [{
              uri,
              mimeType: 'application/json',
              text: JSON.stringify(await this.defiService.getOrderBook()),
            }],
          };

        case 'dashboard':
          return {
            contents: [{
              uri,
              mimeType: 'application/json',
              text: JSON.stringify(await this.analyticsService.getDashboard()),
            }],
          };

        default:
          throw new Error(`Unknown resource: ${resource}`);
      }
    } catch (error) {
      console.error(`Error reading resource ${uri}:`, error);
      throw error;
    }
  }

  /**
   * Start the MCP server
   */
  async start(): Promise<void> {
    const transport = new StdioServerTransport();
    await this.server.connect(transport);
    console.log('isA_Chain MCP Bridge Server started');
  }
}

// Start the server if this file is run directly
if (import.meta.url === `file://${process.argv[1]}`) {
  const server = new IsAChainMCPServer();
  server.start().catch(console.error);
}

export { IsAChainMCPServer };