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
import { getTools } from './tools.js';
import { getResources } from './resources.js';
import { executeToolCall, readResource, BridgeServices } from './handlers.js';

/**
 * MCP Bridge Server for isA_Chain
 * Provides AI agents with blockchain capabilities through MCP protocol
 */
class IsAChainMCPServer {
  private server: Server;
  private services: BridgeServices;

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
    const rpcUrl = process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545';
    const privateKey = process.env.PRIVATE_KEY;
    
    if (!privateKey) {
      throw new Error('PRIVATE_KEY environment variable is required');
    }

    const provider = new ethers.JsonRpcProvider(rpcUrl);
    const wallet = new ethers.Wallet(privateKey, provider);

    this.services = {
      blockchainService: new BlockchainService(provider),
      contractService: new ContractService(wallet),
      walletService: new WalletService(wallet),
      defiService: new DeFiService(wallet),
      nftService: new NFTService(wallet),
      analyticsService: new AnalyticsService(provider),
    };
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

  getTools(): Tool[] {
    return getTools();
  }

  getResources(): Resource[] {
    return getResources();
  }

  async handleToolCall(name: string, args: any): Promise<any> {
    return await executeToolCall(this.services, name, args);
  }

  async handleResourceRead(uri: string): Promise<any> {
    return await readResource(this.services, uri);
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