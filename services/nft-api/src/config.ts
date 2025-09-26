export const config = {
  port: parseInt(process.env.PORT || '8312'),
  blockchain: {
    rpcUrl: process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545',
    chainId: parseInt(process.env.CHAIN_ID || '1337'),
    privateKey: process.env.PRIVATE_KEY || '',
  },
  contracts: {
    isaNFT: process.env.ISA_NFT_CONTRACT_ADDRESS || '0x0000000000000000000000000000000000000003',
    marketplace: process.env.MARKETPLACE_CONTRACT_ADDRESS || '0x0000000000000000000000000000000000000004',
  },
  ipfs: {
    gateway: process.env.IPFS_GATEWAY || 'https://ipfs.io/ipfs',
  },
  consul: {
    host: process.env.CONSUL_HOST || 'localhost',
    port: parseInt(process.env.CONSUL_PORT || '8500'),
  },
  serviceName: 'nft-service',
  serviceVersion: '1.0.0',
};