/**
 * @fileoverview Contract Service - Smart contract interaction service
 * 
 * Provides high-level interfaces for interacting with isA_Chain smart contracts
 */

import { ethers } from 'ethers';

// Contract addresses (would be loaded from environment or config)
const CONTRACT_ADDRESSES = {
  ISA_TOKEN: process.env.ISA_TOKEN_ADDRESS || '0x1234567890123456789012345678901234567890',
  SIMPLE_DEX: process.env.SIMPLE_DEX_ADDRESS || '0x1234567890123456789012345678901234567891',
  GOVERNANCE: process.env.GOVERNANCE_ADDRESS || '0x1234567890123456789012345678901234567892',
  STAKING_POOL: process.env.STAKING_POOL_ADDRESS || '0x1234567890123456789012345678901234567893',
  LENDING_PROTOCOL: process.env.LENDING_PROTOCOL_ADDRESS || '0x1234567890123456789012345678901234567894',
  YIELD_FARMING: process.env.YIELD_FARMING_ADDRESS || '0x1234567890123456789012345678901234567895',
  ISA_NFT: process.env.ISA_NFT_ADDRESS || '0x1234567890123456789012345678901234567896',
  NFT_MARKETPLACE: process.env.NFT_MARKETPLACE_ADDRESS || '0x1234567890123456789012345678901234567897',
  PRIVACY_POOL: process.env.PRIVACY_POOL_ADDRESS || '0x1234567890123456789012345678901234567898',
  PRICE_ORACLE: process.env.PRICE_ORACLE_ADDRESS || '0x1234567890123456789012345678901234567899',
  SPOT_EXCHANGE: process.env.SPOT_EXCHANGE_ADDRESS || '0x1234567890123456789012345678901234567800',
  ORDER_MANAGER: process.env.ORDER_MANAGER_ADDRESS || '0x1234567890123456789012345678901234567801',
};

// Minimal ABIs for essential functions
const ISA_TOKEN_ABI = [
  'function balanceOf(address account) view returns (uint256)',
  'function transfer(address to, uint256 amount) returns (bool)',
  'function approve(address spender, uint256 amount) returns (bool)',
  'function allowance(address owner, address spender) view returns (uint256)',
  'function totalSupply() view returns (uint256)',
  'function decimals() view returns (uint8)',
  'function symbol() view returns (string)',
  'function name() view returns (string)',
];

const SIMPLE_DEX_ABI = [
  'function swapExactTokensForTokens(uint256 amountIn, uint256 amountOutMin, address tokenIn, address tokenOut) returns (uint256)',
  'function addLiquidity(address tokenA, address tokenB, uint256 amountA, uint256 amountB) returns (uint256, uint256, uint256)',
  'function removeLiquidity(address tokenA, address tokenB, uint256 liquidity) returns (uint256, uint256)',
  'function getReserves(address tokenA, address tokenB) view returns (uint256, uint256)',
  'function getAmountOut(uint256 amountIn, address tokenIn, address tokenOut) view returns (uint256)',
];

const STAKING_POOL_ABI = [
  'function stake(uint256 poolId, uint256 amount, uint256 lockDuration)',
  'function withdraw(uint256 poolId, uint256 amount)',
  'function claimRewards(uint256 poolId)',
  'function getStakeInfo(address user, uint256 poolId) view returns (uint256, uint256, uint256, uint256)',
  'function pools(uint256) view returns (address, uint256, uint256, uint256, bool)',
];

const LENDING_PROTOCOL_ABI = [
  'function supply(address asset, uint256 amount)',
  'function withdraw(address asset, uint256 amount)',
  'function borrow(address asset, uint256 amount)',
  'function repayBorrow(address asset, uint256 amount)',
  'function getUserSupplyBalance(address user, address asset) view returns (uint256)',
  'function getUserBorrowBalance(address user, address asset) view returns (uint256)',
];

const NFT_ABI = [
  'function mint(address to, string tokenURI)',
  'function ownerOf(uint256 tokenId) view returns (address)',
  'function tokenURI(uint256 tokenId) view returns (string)',
  'function balanceOf(address owner) view returns (uint256)',
  'function approve(address to, uint256 tokenId)',
  'function transferFrom(address from, address to, uint256 tokenId)',
];

const NFT_MARKETPLACE_ABI = [
  'function listForSale(uint256 tokenId, uint256 price, address paymentToken, uint256 duration)',
  'function buy(uint256 tokenId)',
  'function cancelListing(uint256 tokenId)',
  'function listings(uint256) view returns (address, uint256, address, uint256, uint256, bool)',
];

export interface ContractInfo {
  address: string;
  abi: any[];
  contract: ethers.Contract;
}

export interface TokenInfo {
  address: string;
  name: string;
  symbol: string;
  decimals: number;
  totalSupply: string;
}

export interface PoolInfo {
  id: number;
  stakingToken: string;
  rewardToken: string;
  apy: number;
  totalStaked: string;
  isActive: boolean;
}

/**
 * Contract Service
 * Handles smart contract interactions and provides typed interfaces
 */
export class ContractService {
  private wallet: ethers.Wallet;
  private contracts: Map<string, ContractInfo> = new Map();

  constructor(wallet: ethers.Wallet) {
    this.wallet = wallet;
    this.initializeContracts();
  }

  /**
   * Initialize contract instances
   */
  private initializeContracts(): void {
    const contractConfigs = [
      { name: 'ISA_TOKEN', address: CONTRACT_ADDRESSES.ISA_TOKEN, abi: ISA_TOKEN_ABI },
      { name: 'SIMPLE_DEX', address: CONTRACT_ADDRESSES.SIMPLE_DEX, abi: SIMPLE_DEX_ABI },
      { name: 'STAKING_POOL', address: CONTRACT_ADDRESSES.STAKING_POOL, abi: STAKING_POOL_ABI },
      { name: 'LENDING_PROTOCOL', address: CONTRACT_ADDRESSES.LENDING_PROTOCOL, abi: LENDING_PROTOCOL_ABI },
      { name: 'ISA_NFT', address: CONTRACT_ADDRESSES.ISA_NFT, abi: NFT_ABI },
      { name: 'NFT_MARKETPLACE', address: CONTRACT_ADDRESSES.NFT_MARKETPLACE, abi: NFT_MARKETPLACE_ABI },
    ];

    for (const config of contractConfigs) {
      const contract = new ethers.Contract(config.address, config.abi, this.wallet);
      this.contracts.set(config.name, {
        address: config.address,
        abi: config.abi,
        contract,
      });
    }
  }

  /**
   * Get contract instance by name
   */
  getContract(name: string): ethers.Contract {
    const contractInfo = this.contracts.get(name);
    if (!contractInfo) {
      throw new Error(`Contract ${name} not found`);
    }
    return contractInfo.contract;
  }

  /**
   * Get contract address by name
   */
  getContractAddress(name: string): string {
    const contractInfo = this.contracts.get(name);
    if (!contractInfo) {
      throw new Error(`Contract ${name} not found`);
    }
    return contractInfo.address;
  }

  /**
   * Get token information
   */
  async getTokenInfo(tokenAddress: string): Promise<TokenInfo> {
    try {
      const tokenContract = new ethers.Contract(tokenAddress, ISA_TOKEN_ABI, this.wallet);
      
      const [name, symbol, decimals, totalSupply] = await Promise.all([
        tokenContract.name(),
        tokenContract.symbol(),
        tokenContract.decimals(),
        tokenContract.totalSupply(),
      ]);

      return {
        address: tokenAddress,
        name,
        symbol,
        decimals,
        totalSupply: ethers.formatUnits(totalSupply, decimals),
      };
    } catch (error) {
      console.error('Error getting token info:', error);
      throw error;
    }
  }

  /**
   * Get token balance for an address
   */
  async getTokenBalance(tokenAddress: string, walletAddress: string): Promise<string> {
    try {
      const tokenContract = new ethers.Contract(tokenAddress, ISA_TOKEN_ABI, this.wallet);
      const balance = await tokenContract.balanceOf(walletAddress);
      const decimals = await tokenContract.decimals();
      
      return ethers.formatUnits(balance, decimals);
    } catch (error) {
      console.error('Error getting token balance:', error);
      throw error;
    }
  }

  /**
   * Approve token spending
   */
  async approveToken(
    tokenAddress: string, 
    spenderAddress: string, 
    amount: string
  ): Promise<ethers.TransactionResponse> {
    try {
      const tokenContract = new ethers.Contract(tokenAddress, ISA_TOKEN_ABI, this.wallet);
      const decimals = await tokenContract.decimals();
      const amountWei = ethers.parseUnits(amount, decimals);
      
      return await tokenContract.approve(spenderAddress, amountWei);
    } catch (error) {
      console.error('Error approving token:', error);
      throw error;
    }
  }

  /**
   * Transfer tokens
   */
  async transferToken(
    tokenAddress: string,
    to: string,
    amount: string
  ): Promise<ethers.TransactionResponse> {
    try {
      const tokenContract = new ethers.Contract(tokenAddress, ISA_TOKEN_ABI, this.wallet);
      const decimals = await tokenContract.decimals();
      const amountWei = ethers.parseUnits(amount, decimals);
      
      return await tokenContract.transfer(to, amountWei);
    } catch (error) {
      console.error('Error transferring token:', error);
      throw error;
    }
  }

  /**
   * Get allowance for token spending
   */
  async getTokenAllowance(
    tokenAddress: string,
    owner: string,
    spender: string
  ): Promise<string> {
    try {
      const tokenContract = new ethers.Contract(tokenAddress, ISA_TOKEN_ABI, this.wallet);
      const allowance = await tokenContract.allowance(owner, spender);
      const decimals = await tokenContract.decimals();
      
      return ethers.formatUnits(allowance, decimals);
    } catch (error) {
      console.error('Error getting token allowance:', error);
      throw error;
    }
  }

  /**
   * Estimate gas for contract interaction
   */
  async estimateContractGas(
    contractName: string,
    method: string,
    params: any[]
  ): Promise<{
    gasLimit: string;
    gasPrice: string;
    totalCost: string;
  }> {
    try {
      const contract = this.getContract(contractName);
      const gasLimit = await contract[method].estimateGas(...params);
      const feeData = await this.wallet.provider?.getFeeData();
      const gasPrice = feeData?.gasPrice || 0n;
      
      const totalCost = gasLimit * gasPrice;

      return {
        gasLimit: gasLimit.toString(),
        gasPrice: ethers.formatUnits(gasPrice, 'gwei'),
        totalCost: ethers.formatEther(totalCost),
      };
    } catch (error) {
      console.error('Error estimating contract gas:', error);
      throw error;
    }
  }

  /**
   * Call contract method (read-only)
   */
  async callContract(
    contractName: string,
    method: string,
    params: any[] = []
  ): Promise<any> {
    try {
      const contract = this.getContract(contractName);
      return await contract[method](...params);
    } catch (error) {
      console.error('Error calling contract method:', error);
      throw error;
    }
  }

  /**
   * Execute contract transaction
   */
  async executeContract(
    contractName: string,
    method: string,
    params: any[] = [],
    options: { value?: string; gasLimit?: string } = {}
  ): Promise<ethers.TransactionResponse> {
    try {
      const contract = this.getContract(contractName);
      const txOptions: any = {};
      
      if (options.value) {
        txOptions.value = ethers.parseEther(options.value);
      }
      
      if (options.gasLimit) {
        txOptions.gasLimit = options.gasLimit;
      }

      return await contract[method](...params, txOptions);
    } catch (error) {
      console.error('Error executing contract transaction:', error);
      throw error;
    }
  }

  /**
   * Get contract events
   */
  async getContractEvents(
    contractName: string,
    eventName: string,
    filters: any = {},
    fromBlock: number | 'latest' = 'latest',
    toBlock: number | 'latest' = 'latest'
  ): Promise<ethers.Log[]> {
    try {
      const contract = this.getContract(contractName);
      const eventFilter = contract.filters[eventName](...Object.values(filters));
      
      return await contract.queryFilter(eventFilter, fromBlock, toBlock);
    } catch (error) {
      console.error('Error getting contract events:', error);
      throw error;
    }
  }

  /**
   * Decode contract event logs
   */
  async decodeEventLogs(
    contractName: string,
    logs: ethers.Log[]
  ): Promise<any[]> {
    try {
      const contract = this.getContract(contractName);
      
      return logs.map(log => {
        try {
          return contract.interface.parseLog(log);
        } catch (error) {
          console.warn('Failed to decode log:', error);
          return null;
        }
      }).filter(Boolean);
    } catch (error) {
      console.error('Error decoding event logs:', error);
      throw error;
    }
  }

  /**
   * Check if contract is deployed
   */
  async isContractDeployed(contractName: string): Promise<boolean> {
    try {
      const address = this.getContractAddress(contractName);
      const code = await this.wallet.provider?.getCode(address);
      return code !== '0x' && code !== undefined;
    } catch (error) {
      console.error('Error checking contract deployment:', error);
      return false;
    }
  }

  /**
   * Get contract bytecode
   */
  async getContractBytecode(contractName: string): Promise<string> {
    try {
      const address = this.getContractAddress(contractName);
      return await this.wallet.provider?.getCode(address) || '0x';
    } catch (error) {
      console.error('Error getting contract bytecode:', error);
      throw error;
    }
  }

  /**
   * Listen to contract events
   */
  listenToContractEvent(
    contractName: string,
    eventName: string,
    callback: (event: any) => void,
    filters: any = {}
  ): () => void {
    const contract = this.getContract(contractName);
    const eventFilter = contract.filters[eventName](...Object.values(filters));
    
    contract.on(eventFilter, callback);
    
    // Return cleanup function
    return () => {
      contract.off(eventFilter, callback);
    };
  }

  /**
   * Batch contract calls
   */
  async batchContractCalls(calls: {
    contractName: string;
    method: string;
    params: any[];
  }[]): Promise<any[]> {
    try {
      const promises = calls.map(call => 
        this.callContract(call.contractName, call.method, call.params)
      );
      
      return await Promise.all(promises);
    } catch (error) {
      console.error('Error executing batch contract calls:', error);
      throw error;
    }
  }

  /**
   * Get wallet address
   */
  getWalletAddress(): string {
    return this.wallet.address;
  }

  /**
   * Get all contract addresses
   */
  getAllContractAddresses(): Record<string, string> {
    const addresses: Record<string, string> = {};
    
    for (const [name, info] of this.contracts.entries()) {
      addresses[name] = info.address;
    }
    
    return addresses;
  }

  /**
   * Update contract address (for testing or upgrades)
   */
  updateContractAddress(name: string, newAddress: string): void {
    const contractInfo = this.contracts.get(name);
    if (contractInfo) {
      const newContract = new ethers.Contract(newAddress, contractInfo.abi, this.wallet);
      this.contracts.set(name, {
        ...contractInfo,
        address: newAddress,
        contract: newContract,
      });
    }
  }

  /**
   * Add custom contract
   */
  addCustomContract(name: string, address: string, abi: any[]): void {
    const contract = new ethers.Contract(address, abi, this.wallet);
    this.contracts.set(name, { address, abi, contract });
  }
}