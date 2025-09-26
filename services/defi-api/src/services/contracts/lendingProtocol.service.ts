import { ethers } from 'ethers';
import { BlockchainService } from '../blockchain.service';
import { logger } from '../../utils/logger';
import { config } from '../../config';

interface SupplyParams {
  asset: string;
  amount: string;
  userAddress: string;
}

interface BorrowParams {
  asset: string;
  amount: string;
  userAddress: string;
}

interface RepayParams {
  asset: string;
  amount: string;
  userAddress: string;
}

interface WithdrawParams {
  asset: string;
  amount: string;
  userAddress: string;
}

export class LendingProtocolService {
  private contract: ethers.Contract | null = null;
  private abi: any[] = []; // Placeholder ABI

  constructor(private blockchain: BlockchainService) {}

  private async getContract(): Promise<ethers.Contract> {
    if (!this.contract) {
      this.contract = await this.blockchain.getContract(
        config.contracts.lendingProtocol,
        this.abi
      );
    }
    return this.contract;
  }

  async getAllMarkets() {
    try {
      const contract = await this.getContract();
      
      const marketCount = await contract.getMarketsCount();
      const markets = [];
      
      for (let i = 0; i < marketCount; i++) {
        const market = await contract.markets(i);
        const reserves = await contract.getReserveData(market.asset);
        
        markets.push({
          asset: market.asset,
          symbol: await this.getTokenSymbol(market.asset),
          supplyRate: ethers.formatEther(reserves.currentSupplyRate),
          borrowRate: ethers.formatEther(reserves.currentBorrowRate),
          totalSupply: ethers.formatEther(reserves.totalSupply),
          totalBorrow: ethers.formatEther(reserves.totalBorrow),
          utilizationRate: this.calculateUtilizationRate(reserves.totalSupply, reserves.totalBorrow),
          collateralFactor: ethers.formatEther(market.collateralFactor),
          liquidationThreshold: ethers.formatEther(market.liquidationThreshold)
        });
      }
      
      return markets;
    } catch (error) {
      logger.error('Failed to get all markets:', error);
      throw error;
    }
  }

  async getUserAccountData(address: string) {
    try {
      const contract = await this.getContract();
      
      const accountData = await contract.getUserAccountData(address);
      
      return {
        totalCollateralETH: ethers.formatEther(accountData.totalCollateralETH),
        totalDebtETH: ethers.formatEther(accountData.totalDebtETH),
        availableBorrowsETH: ethers.formatEther(accountData.availableBorrowsETH),
        currentLiquidationThreshold: accountData.currentLiquidationThreshold.toString(),
        ltv: accountData.ltv.toString(),
        healthFactor: ethers.formatEther(accountData.healthFactor)
      };
    } catch (error) {
      logger.error('Failed to get user account data:', error);
      throw error;
    }
  }

  async getUserReserveData(asset: string, address: string) {
    try {
      const contract = await this.getContract();
      
      const reserveData = await contract.getUserReserveData(asset, address);
      
      return {
        currentATokenBalance: ethers.formatEther(reserveData.currentATokenBalance),
        currentStableDebt: ethers.formatEther(reserveData.currentStableDebt),
        currentVariableDebt: ethers.formatEther(reserveData.currentVariableDebt),
        principalStableDebt: ethers.formatEther(reserveData.principalStableDebt),
        scaledVariableDebt: ethers.formatEther(reserveData.scaledVariableDebt),
        stableBorrowRate: ethers.formatEther(reserveData.stableBorrowRate),
        liquidityRate: ethers.formatEther(reserveData.liquidityRate),
        usageAsCollateralEnabled: reserveData.usageAsCollateralEnabled
      };
    } catch (error) {
      logger.error('Failed to get user reserve data:', error);
      throw error;
    }
  }

  async supply(params: SupplyParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.supply(
        params.asset,
        ethers.parseEther(params.amount),
        params.userAddress,
        0, // referralCode
        { from: params.userAddress }
      );
      
      logger.info(`Supply transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to supply:', error);
      throw error;
    }
  }

  async withdraw(params: WithdrawParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.withdraw(
        params.asset,
        ethers.parseEther(params.amount),
        params.userAddress,
        { from: params.userAddress }
      );
      
      logger.info(`Withdraw transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to withdraw:', error);
      throw error;
    }
  }

  async borrow(params: BorrowParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.borrow(
        params.asset,
        ethers.parseEther(params.amount),
        2, // interestRateMode (1 = stable, 2 = variable)
        0, // referralCode
        params.userAddress,
        { from: params.userAddress }
      );
      
      logger.info(`Borrow transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to borrow:', error);
      throw error;
    }
  }

  async repay(params: RepayParams): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.repay(
        params.asset,
        ethers.parseEther(params.amount),
        2, // interestRateMode (1 = stable, 2 = variable)
        params.userAddress,
        { from: params.userAddress }
      );
      
      logger.info(`Repay transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to repay:', error);
      throw error;
    }
  }

  async setUserUseReserveAsCollateral(asset: string, useAsCollateral: boolean, userAddress: string): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.setUserUseReserveAsCollateral(
        asset,
        useAsCollateral,
        { from: userAddress }
      );
      
      logger.info(`Set collateral transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to set collateral:', error);
      throw error;
    }
  }

  async liquidationCall(
    collateralAsset: string,
    debtAsset: string,
    user: string,
    debtToCover: string,
    receiveAToken: boolean,
    liquidatorAddress: string
  ): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const tx = await contract.liquidationCall(
        collateralAsset,
        debtAsset,
        user,
        ethers.parseEther(debtToCover),
        receiveAToken,
        { from: liquidatorAddress }
      );
      
      logger.info(`Liquidation transaction sent: ${tx.hash}`);
      return tx.hash;
    } catch (error) {
      logger.error('Failed to execute liquidation:', error);
      throw error;
    }
  }

  async getHealthFactor(address: string): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const accountData = await contract.getUserAccountData(address);
      return ethers.formatEther(accountData.healthFactor);
    } catch (error) {
      logger.error('Failed to get health factor:', error);
      throw error;
    }
  }

  async getLiquidationThreshold(asset: string): Promise<string> {
    try {
      const contract = await this.getContract();
      
      const configData = await contract.getConfiguration(asset);
      return ethers.formatEther(configData.liquidationThreshold);
    } catch (error) {
      logger.error('Failed to get liquidation threshold:', error);
      throw error;
    }
  }

  async getUserPositions(address: string) {
    try {
      const contract = await this.getContract();
      const marketCount = await contract.getMarketsCount();
      const positions = {
        supplies: [],
        borrows: []
      };
      
      for (let i = 0; i < marketCount; i++) {
        const market = await contract.markets(i);
        const userReserveData = await contract.getUserReserveData(market.asset, address);
        
        if (userReserveData.currentATokenBalance > 0) {
          positions.supplies.push({
            asset: market.asset,
            symbol: await this.getTokenSymbol(market.asset),
            balance: ethers.formatEther(userReserveData.currentATokenBalance),
            isCollateral: userReserveData.usageAsCollateralEnabled
          });
        }
        
        if (userReserveData.currentVariableDebt > 0) {
          positions.borrows.push({
            asset: market.asset,
            symbol: await this.getTokenSymbol(market.asset),
            debt: ethers.formatEther(userReserveData.currentVariableDebt),
            borrowRate: ethers.formatEther(userReserveData.stableBorrowRate)
          });
        }
      }
      
      return positions;
    } catch (error) {
      logger.error('Failed to get user positions:', error);
      throw error;
    }
  }

  private calculateUtilizationRate(totalSupply: bigint, totalBorrow: bigint): number {
    if (totalSupply === BigInt(0)) {
      return 0;
    }
    
    const utilization = (Number(totalBorrow) / Number(totalSupply)) * 100;
    return Math.round(utilization * 100) / 100;
  }

  private async getTokenSymbol(tokenAddress: string): Promise<string> {
    try {
      // This would typically query the token contract for its symbol
      // For now, return a placeholder
      return 'TOKEN';
    } catch (error) {
      logger.error('Failed to get token symbol:', error);
      return 'UNKNOWN';
    }
  }
}