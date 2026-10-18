import { Resource } from '@modelcontextprotocol/sdk/types.js';

/**
 * Define available resources for AI agents
 */
export function getResources(): Resource[] {
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
