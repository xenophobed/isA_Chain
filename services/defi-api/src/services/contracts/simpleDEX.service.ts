import { ethers } from 'ethers';
import { BlockchainService } from '../blockchain.service';
import { logger } from '../../utils/logger';
import SimpleDEXABI from '../../../../../contracts/abi/SimpleDEX.json';
import { config } from '../../config';

interface SwapParams {
  tokenIn: string;
  tokenOut: string;
  amountIn: string;
  minAmountOut: string;
  userAddress: string;
}

interface LiquidityParams {
  tokenA: string;
  tokenB: string;
  amountA: string;
  amountB: string;
  userAddress: string;
}

export class SimpleDEXService {
  private contract: ethers.Contract | null = null;

  constructor(private blockchain: BlockchainService) {}

  private async getContract(): Promise<ethers.Contract> {
    if (!this.contract) {
      this.contract = await this.blockchain.getContract(
        config.contracts.simpleDEX,
        SimpleDEXABI.abi || SimpleDEXABI
      );
    }
    return this.contract;
  }

  async getAllPools() {
    try {
      const contract = await this.getContract();
      
      // This would depend on your actual SimpleDEX implementation
      // Example: getting pool count and iterating through pools
      const poolCount = await contract.getPoolCount();
      const pools = [];
      
      for (let i = 0; i < poolCount; i++) {
        const pool = await contract.pools(i);
        pools.push({
          id: i,
          tokenA: pool.tokenA,
          tokenB: pool.tokenB,
          reserveA: pool.reserveA.toString(),
          reserveB: pool.reserveB.toString(),
          totalLiquidity: pool.totalLiquidity.toString(),
          fee: pool.fee.toString()
        });
      }
      
      return pools;
    } catch (error) {
      logger.error('Failed to get pools:', error);
      throw error;
    }
  }

  async getSwapQuote(tokenIn: string, tokenOut: string, amountIn: string) {
    try {
      const contract = await this.getContract();
      
      // Call the contract's getAmountOut function
      const amountOut = await contract.getAmountOut(
        ethers.parseEther(amountIn),
        tokenIn,
        tokenOut
      );
      
      // Calculate price impact (simplified)
      const priceImpact = this.calculatePriceImpact(amountIn, amountOut.toString());
      
      return {
        estimatedOut: ethers.formatEther(amountOut),
        priceImpact,
        fee: '0.3', // 0.3% fee
        route: [tokenIn, tokenOut]
      };
    } catch (error) {
      logger.error('Failed to get swap quote:', error);
      throw error;
    }
  }

  async executeSwap(params: SwapParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      // Build transaction
      const tx = await contract.swap(
        params.tokenIn,
        params.tokenOut,
        ethers.parseEther(params.amountIn),
        ethers.parseEther(params.minAmountOut),
        params.userAddress,
        Math.floor(Date.now() / 1000) + 3600 // 1 hour deadline
      );
      
      logger.info(`Swap transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to execute swap:', error);
      throw error;
    }
  }

  async addLiquidity(params: LiquidityParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.addLiquidity(
        params.tokenA,
        params.tokenB,
        ethers.parseEther(params.amountA),
        ethers.parseEther(params.amountB),
        ethers.parseEther(params.amountA).mul(95).div(100), // 5% slippage
        ethers.parseEther(params.amountB).mul(95).div(100), // 5% slippage
        params.userAddress,
        Math.floor(Date.now() / 1000) + 3600 // 1 hour deadline
      );
      
      logger.info(`Add liquidity transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to add liquidity:', error);
      throw error;
    }
  }

  async removeLiquidity(params: {
    poolId: number;
    lpTokenAmount: string;
    userAddress: string;
  }): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.removeLiquidity(
        params.poolId,
        ethers.parseEther(params.lpTokenAmount),
        0, // min amount A
        0, // min amount B
        params.userAddress,
        Math.floor(Date.now() / 1000) + 3600 // 1 hour deadline
      );
      
      logger.info(`Remove liquidity transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to remove liquidity:', error);
      throw error;
    }
  }

  async getUserLiquidityPositions(address: string) {
    try {
      const contract = await this.getContract();
      
      // Get user's LP token balances for each pool
      const poolCount = await contract.getPoolCount();
      const positions = [];
      
      for (let i = 0; i < poolCount; i++) {
        const lpBalance = await contract.getLPBalance(i, address);
        if (lpBalance > 0) {
          const pool = await contract.pools(i);
          positions.push({
            poolId: i,
            tokenA: pool.tokenA,
            tokenB: pool.tokenB,
            lpTokens: ethers.formatEther(lpBalance),
            share: await this.calculatePoolShare(i, lpBalance)
          });
        }
      }
      
      return positions;
    } catch (error) {
      logger.error('Failed to get liquidity positions:', error);
      throw error;
    }
  }

  private calculatePriceImpact(amountIn: string, amountOut: string): number {
    // Simplified price impact calculation
    // In production, this would be more sophisticated
    const inValue = parseFloat(amountIn);
    const outValue = parseFloat(amountOut);
    const expectedOut = inValue; // Assuming 1:1 for simplicity
    const impact = ((expectedOut - outValue) / expectedOut) * 100;
    return Math.abs(impact);
  }

  private async calculatePoolShare(poolId: number, lpBalance: bigint): Promise<string> {
    try {
      const contract = await this.getContract();
      const pool = await contract.pools(poolId);
      const totalLiquidity = pool.totalLiquidity;
      
      if (totalLiquidity === BigInt(0)) {
        return '0';
      }
      
      const share = (lpBalance * BigInt(10000)) / totalLiquidity;
      return (Number(share) / 100).toFixed(2) + '%';
    } catch (error) {
      logger.error('Failed to calculate pool share:', error);
      return '0%';
    }
  }
}