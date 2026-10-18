import { Request, Response } from 'express';
import { SimpleDEXService } from '../../services/contracts/simpleDEX.service';
import { logger } from '../../utils/logger';

export async function getPools(simpleDEX: SimpleDEXService, req: Request, res: Response) {
  try {
    const pools = await simpleDEX.getAllPools();
    res.json({
      success: true,
      data: pools,
      timestamp: Date.now()
    });
  } catch (error) {
    logger.error('Failed to get pools:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch pools'
    });
  }
}

export async function getSwapQuote(simpleDEX: SimpleDEXService, req: Request, res: Response) {
  try {
    const { tokenIn, tokenOut, amountIn } = req.body;
    
    if (!tokenIn || !tokenOut || !amountIn) {
      return res.status(400).json({
        success: false,
        error: 'Missing required parameters: tokenIn, tokenOut, amountIn'
      });
    }

    const quote = await simpleDEX.getSwapQuote(tokenIn, tokenOut, amountIn);
    
    res.json({
      success: true,
      data: {
        tokenIn,
        tokenOut,
        amountIn,
        estimatedOut: quote.estimatedOut,
        priceImpact: quote.priceImpact,
        fee: quote.fee,
        route: quote.route
      }
    });
  } catch (error) {
    logger.error('Failed to get swap quote:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to calculate swap quote'
    });
  }
}

export async function executeSwap(simpleDEX: SimpleDEXService, req: Request, res: Response) {
  try {
    const { tokenIn, tokenOut, amountIn, minAmountOut, userAddress } = req.body;
    
    if (!tokenIn || !tokenOut || !amountIn || !minAmountOut || !userAddress) {
      return res.status(400).json({
        success: false,
        error: 'Missing required parameters'
      });
    }

    const txHash = await simpleDEX.executeSwap({
      tokenIn,
      tokenOut,
      amountIn,
      minAmountOut,
      userAddress
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Swap transaction submitted'
      }
    });
  } catch (error) {
    logger.error('Failed to execute swap:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to execute swap'
    });
  }
}

export async function addLiquidity(simpleDEX: SimpleDEXService, req: Request, res: Response) {
  try {
    const { tokenA, tokenB, amountA, amountB, userAddress } = req.body;

    const txHash = await simpleDEX.addLiquidity({
      tokenA,
      tokenB,
      amountA,
      amountB,
      userAddress
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Liquidity addition transaction submitted'
      }
    });
  } catch (error) {
    logger.error('Failed to add liquidity:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to add liquidity'
    });
  }
}

export async function removeLiquidity(simpleDEX: SimpleDEXService, req: Request, res: Response) {
  try {
    const { poolId, lpTokenAmount, userAddress } = req.body;

    const txHash = await simpleDEX.removeLiquidity({
      poolId,
      lpTokenAmount,
      userAddress
    });

    res.json({
      success: true,
      data: {
        transactionHash: txHash,
        status: 'pending',
        message: 'Liquidity removal transaction submitted'
      }
    });
  } catch (error) {
    logger.error('Failed to remove liquidity:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to remove liquidity'
    });
  }
}

export async function getLiquidityPositions(simpleDEX: SimpleDEXService, req: Request, res: Response) {
  try {
    const { address } = req.params;
    
    const positions = await simpleDEX.getUserLiquidityPositions(address);
    
    res.json({
      success: true,
      data: positions
    });
  } catch (error) {
    logger.error('Failed to get liquidity positions:', error);
    res.status(500).json({
      success: false,
      error: 'Failed to fetch liquidity positions'
    });
  }
}
