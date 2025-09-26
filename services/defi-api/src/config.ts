import * as dotenv from 'dotenv';

dotenv.config();

export const config = {
  // Server configuration
  server: {
    port: parseInt(process.env.PORT || '8311'),
    environment: process.env.NODE_ENV || 'development'
  },

  // Blockchain configuration
  blockchain: {
    rpcUrl: process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545',
    chainId: parseInt(process.env.CHAIN_ID || '1337'),
    privateKey: process.env.PRIVATE_KEY || '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80', // Default Hardhat account #0
  },

  // Contract addresses (mock addresses for testing)
  contracts: {
    simpleDEX: process.env.CONTRACT_SIMPLE_DEX || '0x5FbDB2315678afecb367f032d93F642f64180aa3',
    stakingPool: process.env.CONTRACT_STAKING_POOL || '0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512',
    yieldFarming: process.env.CONTRACT_YIELD_FARMING || '0x9fE46736679d2D9a65F0992F2272dE9f3c7fa6e0',
    lendingProtocol: process.env.CONTRACT_LENDING_PROTOCOL || '0xCf7Ed3AccA5a467e9e704C703E8D87F634fB0Fc9',
    isaToken: process.env.CONTRACT_ISA_TOKEN || '0xDc64a140Aa3E981100a9becA4E685f962f0cF6C9'
  },

  // Consul configuration
  consul: {
    host: process.env.CONSUL_HOST || 'localhost',
    port: parseInt(process.env.CONSUL_PORT || '8500'),
    secure: process.env.CONSUL_SECURE === 'true'
  },

  // Gateway configuration
  gateway: {
    url: process.env.GATEWAY_URL || 'http://localhost:8000'
  },

  // Logging configuration
  logging: {
    level: process.env.LOG_LEVEL || 'info',
    format: process.env.LOG_FORMAT || 'json'
  },

  // API limits
  api: {
    rateLimitWindow: parseInt(process.env.RATE_LIMIT_WINDOW || '900000'), // 15 minutes
    rateLimitMax: parseInt(process.env.RATE_LIMIT_MAX || '100')
  }
};