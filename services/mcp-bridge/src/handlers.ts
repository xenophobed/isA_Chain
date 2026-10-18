import { BlockchainService } from './services/blockchain.js';
import { ContractService } from './services/contracts.js';
import { WalletService } from './services/wallet.js';
import { DeFiService } from './services/defi.js';
import { NFTService } from './services/nft.js';
import { AnalyticsService } from './services/analytics.js';

export interface BridgeServices {
  blockchainService: BlockchainService;
  contractService: ContractService;
  walletService: WalletService;
  defiService: DeFiService;
  nftService: NFTService;
  analyticsService: AnalyticsService;
}

/**
 * Handle tool calls from AI agents
 */
export async function executeToolCall(services: BridgeServices, name: string, args: any): Promise<any> {
  const { walletService, defiService, nftService, analyticsService } = services;
  try {
    switch (name) {
      // Wallet operations
      case 'get_wallet_balance':
        return await walletService.getBalance(args.tokenAddress, args.walletAddress);
      
      case 'send_transaction':
        return await walletService.sendTransaction(args.to, args.amount, args.tokenAddress);

      // DeFi operations
      case 'swap_tokens':
        return await defiService.swapTokens(
          args.tokenIn,
          args.tokenOut,
          args.amountIn,
          args.minAmountOut,
          args.slippage
        );

      case 'add_liquidity':
        return await defiService.addLiquidity(
          args.tokenA,
          args.tokenB,
          args.amountA,
          args.amountB
        );

      case 'stake_tokens':
        return await defiService.stakeTokens(args.poolId, args.amount, args.lockDuration);

      case 'lend_tokens':
        return await defiService.lendTokens(args.asset, args.amount);

      case 'borrow_tokens':
        return await defiService.borrowTokens(args.asset, args.amount, args.collateralAsset);

      // NFT operations
      case 'mint_nft':
        return await nftService.mintNFT(args.to, args.tokenURI, args.quantity);

      case 'list_nft_for_sale':
        return await nftService.listForSale(
          args.tokenId,
          args.price,
          args.paymentToken,
          args.duration
        );

      case 'buy_nft':
        return await nftService.buyNFT(args.tokenId, args.maxPrice);

      // Analytics
      case 'get_portfolio_value':
        return await analyticsService.getPortfolioValue(
          args.walletAddress,
          args.includeStaking,
          args.includeLending,
          args.includeNFTs
        );

      case 'get_yield_opportunities':
        return await analyticsService.getYieldOpportunities(
          args.asset,
          args.amount,
          args.riskLevel,
          args.minAPY
        );

      case 'analyze_transaction':
        return await analyticsService.analyzeTransaction(
          args.txHash,
          args.includeGasAnalysis,
          args.includePriceImpact
        );

      // Trading
      case 'place_limit_order':
        return await defiService.placeLimitOrder(
          args.pair,
          args.side,
          args.amount,
          args.price,
          args.timeInForce
        );

      case 'set_stop_loss':
        return await defiService.setStopLoss(
          args.pair,
          args.side,
          args.amount,
          args.stopPrice,
          args.limitPrice
        );

      // Privacy
      case 'private_transfer':
        return await defiService.privateTransfer(
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
export async function readResource(services: BridgeServices, uri: string): Promise<any> {
  const { blockchainService, defiService, nftService, analyticsService } = services;
  try {
    const [, , resource] = uri.split('/');

    switch (resource) {
      case 'status':
        return {
          contents: [{
            uri,
            mimeType: 'application/json',
            text: JSON.stringify(await blockchainService.getStatus()),
          }],
        };

      case 'pools':
        return {
          contents: [{
            uri,
            mimeType: 'application/json',
            text: JSON.stringify(await defiService.getAllPools()),
          }],
        };

      case 'collections':
        return {
          contents: [{
            uri,
            mimeType: 'application/json',
            text: JSON.stringify(await nftService.getAllCollections()),
          }],
        };

      case 'orderbook':
        return {
          contents: [{
            uri,
            mimeType: 'application/json',
            text: JSON.stringify(await defiService.getOrderBook()),
          }],
        };

      case 'dashboard':
        return {
          contents: [{
            uri,
            mimeType: 'application/json',
            text: JSON.stringify(await analyticsService.getDashboard()),
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
