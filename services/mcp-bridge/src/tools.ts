import { Tool } from '@modelcontextprotocol/sdk/types.js';

/**
 * Define available tools for AI agents
 */
export function getTools(): Tool[] {
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
