// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";

/**
 * @title SimpleDEX
 * @dev Basic decentralized exchange for token swapping
 * 
 * Features:
 * - Token-to-token swapping
 * - Automated market maker (constant product formula)
 * - Liquidity provision and rewards
 * - Fee collection
 * - Emergency pause functionality
 */
contract SimpleDEX is Ownable, ReentrancyGuard, Pausable {
    using SafeERC20 for IERC20;
    
    // Trading pair structure
    struct TradingPair {
        address tokenA;
        address tokenB;
        uint256 reserveA;
        uint256 reserveB;
        uint256 totalLiquidity;
        uint256 feeRate; // in basis points (100 = 1%)
        bool active;
    }
    
    // Liquidity provider position
    struct LiquidityPosition {
        uint256 liquidity;
        uint256 rewardDebt;
        uint256 lastDepositTime;
    }
    
    // State variables
    mapping(bytes32 => TradingPair) public tradingPairs;
    mapping(bytes32 => mapping(address => LiquidityPosition)) public liquidityPositions;
    mapping(address => bool) public supportedTokens;
    
    bytes32[] public allPairs;
    uint256 public defaultFeeRate = 300; // 3%
    uint256 public protocolFeeRate = 30; // 0.3% (10% of trading fees)
    address public feeCollector;
    uint256 public minimumLiquidity = 1000;
    
    // Events
    event PairCreated(bytes32 indexed pairId, address indexed tokenA, address indexed tokenB);
    event LiquidityAdded(bytes32 indexed pairId, address indexed provider, uint256 amountA, uint256 amountB, uint256 liquidity);
    event LiquidityRemoved(bytes32 indexed pairId, address indexed provider, uint256 amountA, uint256 amountB, uint256 liquidity);
    event TokensSwapped(bytes32 indexed pairId, address indexed trader, address tokenIn, address tokenOut, uint256 amountIn, uint256 amountOut);
    event FeeRateUpdated(bytes32 indexed pairId, uint256 oldRate, uint256 newRate);
    event ProtocolFeeUpdated(uint256 oldRate, uint256 newRate);
    event TokenSupported(address indexed token, bool supported);
    
    /**
     * @dev Constructor
     * @param _feeCollector Address to collect protocol fees
     */
    constructor(address _feeCollector) {
        require(_feeCollector != address(0), "SimpleDEX: fee collector cannot be zero");
        feeCollector = _feeCollector;
        _transferOwnership(msg.sender);
    }
    
    /**
     * @dev Create a new trading pair
     * @param tokenA First token address
     * @param tokenB Second token address
     * @param feeRate Fee rate in basis points
     */
    function createPair(
        address tokenA,
        address tokenB,
        uint256 feeRate
    ) external onlyOwner returns (bytes32 pairId) {
        require(tokenA != address(0) && tokenB != address(0), "SimpleDEX: invalid token address");
        require(tokenA != tokenB, "SimpleDEX: identical tokens");
        require(supportedTokens[tokenA] && supportedTokens[tokenB], "SimpleDEX: tokens not supported");
        require(feeRate <= 1000, "SimpleDEX: fee rate too high"); // Max 10%
        
        // Ensure consistent ordering
        (address token0, address token1) = tokenA < tokenB ? (tokenA, tokenB) : (tokenB, tokenA);
        pairId = keccak256(abi.encodePacked(token0, token1));
        
        require(!tradingPairs[pairId].active, "SimpleDEX: pair already exists");
        
        tradingPairs[pairId] = TradingPair({
            tokenA: token0,
            tokenB: token1,
            reserveA: 0,
            reserveB: 0,
            totalLiquidity: 0,
            feeRate: feeRate > 0 ? feeRate : defaultFeeRate,
            active: true
        });
        
        allPairs.push(pairId);
        
        emit PairCreated(pairId, token0, token1);
        return pairId;
    }
    
    /**
     * @dev Add liquidity to a trading pair
     * @param pairId Trading pair identifier
     * @param amountADesired Desired amount of token A
     * @param amountBDesired Desired amount of token B
     * @param amountAMin Minimum amount of token A
     * @param amountBMin Minimum amount of token B
     */
    function addLiquidity(
        bytes32 pairId,
        uint256 amountADesired,
        uint256 amountBDesired,
        uint256 amountAMin,
        uint256 amountBMin
    ) external nonReentrant whenNotPaused returns (uint256 amountA, uint256 amountB, uint256 liquidity) {
        TradingPair storage pair = tradingPairs[pairId];
        require(pair.active, "SimpleDEX: pair not active");
        
        (amountA, amountB) = _calculateOptimalAmounts(pair, amountADesired, amountBDesired, amountAMin, amountBMin);
        
        if (pair.totalLiquidity == 0) {
            // First liquidity provision
            liquidity = _sqrt(amountA * amountB) - minimumLiquidity;
            require(liquidity > 0, "SimpleDEX: insufficient liquidity minted");
        } else {
            // Subsequent liquidity provisions
            uint256 liquidityA = (amountA * pair.totalLiquidity) / pair.reserveA;
            uint256 liquidityB = (amountB * pair.totalLiquidity) / pair.reserveB;
            liquidity = liquidityA < liquidityB ? liquidityA : liquidityB;
        }
        
        require(liquidity > 0, "SimpleDEX: insufficient liquidity minted");
        
        // Update reserves and liquidity
        pair.reserveA += amountA;
        pair.reserveB += amountB;
        pair.totalLiquidity += liquidity;
        
        // Update user position
        LiquidityPosition storage position = liquidityPositions[pairId][msg.sender];
        position.liquidity += liquidity;
        position.lastDepositTime = block.timestamp;
        
        // Transfer tokens
        IERC20(pair.tokenA).safeTransferFrom(msg.sender, address(this), amountA);
        IERC20(pair.tokenB).safeTransferFrom(msg.sender, address(this), amountB);
        
        emit LiquidityAdded(pairId, msg.sender, amountA, amountB, liquidity);
    }
    
    /**
     * @dev Remove liquidity from a trading pair
     * @param pairId Trading pair identifier
     * @param liquidity Amount of liquidity tokens to remove
     * @param amountAMin Minimum amount of token A to receive
     * @param amountBMin Minimum amount of token B to receive
     */
    function removeLiquidity(
        bytes32 pairId,
        uint256 liquidity,
        uint256 amountAMin,
        uint256 amountBMin
    ) external nonReentrant returns (uint256 amountA, uint256 amountB) {
        TradingPair storage pair = tradingPairs[pairId];
        require(pair.active, "SimpleDEX: pair not active");
        
        LiquidityPosition storage position = liquidityPositions[pairId][msg.sender];
        require(position.liquidity >= liquidity, "SimpleDEX: insufficient liquidity");
        
        // Calculate token amounts to return
        amountA = (liquidity * pair.reserveA) / pair.totalLiquidity;
        amountB = (liquidity * pair.reserveB) / pair.totalLiquidity;
        
        require(amountA >= amountAMin && amountB >= amountBMin, "SimpleDEX: insufficient output");
        
        // Update reserves and liquidity
        pair.reserveA -= amountA;
        pair.reserveB -= amountB;
        pair.totalLiquidity -= liquidity;
        position.liquidity -= liquidity;
        
        // Transfer tokens
        IERC20(pair.tokenA).safeTransfer(msg.sender, amountA);
        IERC20(pair.tokenB).safeTransfer(msg.sender, amountB);
        
        emit LiquidityRemoved(pairId, msg.sender, amountA, amountB, liquidity);
    }
    
    /**
     * @dev Swap tokens
     * @param pairId Trading pair identifier
     * @param tokenIn Input token address
     * @param amountIn Amount of input tokens
     * @param amountOutMin Minimum amount of output tokens
     */
    function swap(
        bytes32 pairId,
        address tokenIn,
        uint256 amountIn,
        uint256 amountOutMin
    ) external nonReentrant whenNotPaused returns (uint256 amountOut) {
        TradingPair storage pair = tradingPairs[pairId];
        require(pair.active, "SimpleDEX: pair not active");
        require(tokenIn == pair.tokenA || tokenIn == pair.tokenB, "SimpleDEX: invalid token");
        require(amountIn > 0, "SimpleDEX: insufficient input amount");
        
        bool isTokenA = tokenIn == pair.tokenA;
        address tokenOut = isTokenA ? pair.tokenB : pair.tokenA;
        uint256 reserveIn = isTokenA ? pair.reserveA : pair.reserveB;
        uint256 reserveOut = isTokenA ? pair.reserveB : pair.reserveA;
        
        // Calculate output amount with fee
        amountOut = _getAmountOut(amountIn, reserveIn, reserveOut, pair.feeRate);
        require(amountOut >= amountOutMin, "SimpleDEX: insufficient output amount");
        require(amountOut < reserveOut, "SimpleDEX: insufficient liquidity");
        
        // Update reserves
        if (isTokenA) {
            pair.reserveA += amountIn;
            pair.reserveB -= amountOut;
        } else {
            pair.reserveB += amountIn;
            pair.reserveA -= amountOut;
        }
        
        // Transfer tokens
        IERC20(tokenIn).safeTransferFrom(msg.sender, address(this), amountIn);
        IERC20(tokenOut).safeTransfer(msg.sender, amountOut);
        
        // Collect protocol fee
        uint256 protocolFee = (amountIn * protocolFeeRate) / 10000;
        if (protocolFee > 0) {
            IERC20(tokenIn).safeTransfer(feeCollector, protocolFee);
        }
        
        emit TokensSwapped(pairId, msg.sender, tokenIn, tokenOut, amountIn, amountOut);
    }
    
    /**
     * @dev Get swap output amount
     * @param amountIn Input amount
     * @param reserveIn Input token reserve
     * @param reserveOut Output token reserve
     * @param feeRate Fee rate in basis points
     */
    function getAmountOut(
        uint256 amountIn,
        uint256 reserveIn,
        uint256 reserveOut,
        uint256 feeRate
    ) external pure returns (uint256) {
        return _getAmountOut(amountIn, reserveIn, reserveOut, feeRate);
    }
    
    /**
     * @dev Get pair information
     * @param pairId Trading pair identifier
     */
    function getPairInfo(bytes32 pairId) external view returns (TradingPair memory) {
        return tradingPairs[pairId];
    }
    
    /**
     * @dev Get user liquidity position
     * @param pairId Trading pair identifier
     * @param user User address
     */
    function getUserPosition(bytes32 pairId, address user) external view returns (LiquidityPosition memory) {
        return liquidityPositions[pairId][user];
    }
    
    /**
     * @dev Support or unsupport a token
     * @param token Token address
     * @param supported Whether the token is supported
     */
    function setSupportedToken(address token, bool supported) external onlyOwner {
        supportedTokens[token] = supported;
        emit TokenSupported(token, supported);
    }
    
    /**
     * @dev Update protocol fee rate
     * @param newRate New fee rate in basis points
     */
    function setProtocolFeeRate(uint256 newRate) external onlyOwner {
        require(newRate <= 100, "SimpleDEX: protocol fee too high"); // Max 1%
        uint256 oldRate = protocolFeeRate;
        protocolFeeRate = newRate;
        emit ProtocolFeeUpdated(oldRate, newRate);
    }
    
    /**
     * @dev Update pair fee rate
     * @param pairId Trading pair identifier
     * @param newRate New fee rate in basis points
     */
    function setPairFeeRate(bytes32 pairId, uint256 newRate) external onlyOwner {
        require(newRate <= 1000, "SimpleDEX: fee rate too high"); // Max 10%
        TradingPair storage pair = tradingPairs[pairId];
        require(pair.active, "SimpleDEX: pair not active");
        
        uint256 oldRate = pair.feeRate;
        pair.feeRate = newRate;
        emit FeeRateUpdated(pairId, oldRate, newRate);
    }
    
    /**
     * @dev Pause the contract
     */
    function pause() external onlyOwner {
        _pause();
    }
    
    /**
     * @dev Unpause the contract
     */
    function unpause() external onlyOwner {
        _unpause();
    }
    
    // Internal functions
    function _getAmountOut(
        uint256 amountIn,
        uint256 reserveIn,
        uint256 reserveOut,
        uint256 feeRate
    ) internal pure returns (uint256) {
        require(amountIn > 0 && reserveIn > 0 && reserveOut > 0, "SimpleDEX: invalid reserves");
        
        uint256 amountInWithFee = amountIn * (10000 - feeRate);
        uint256 numerator = amountInWithFee * reserveOut;
        uint256 denominator = (reserveIn * 10000) + amountInWithFee;
        
        return numerator / denominator;
    }
    
    function _calculateOptimalAmounts(
        TradingPair storage pair,
        uint256 amountADesired,
        uint256 amountBDesired,
        uint256 amountAMin,
        uint256 amountBMin
    ) internal view returns (uint256 amountA, uint256 amountB) {
        if (pair.reserveA == 0 && pair.reserveB == 0) {
            // First liquidity provision
            return (amountADesired, amountBDesired);
        }
        
        uint256 amountBOptimal = (amountADesired * pair.reserveB) / pair.reserveA;
        if (amountBOptimal <= amountBDesired) {
            require(amountBOptimal >= amountBMin, "SimpleDEX: insufficient B amount");
            return (amountADesired, amountBOptimal);
        }
        
        uint256 amountAOptimal = (amountBDesired * pair.reserveA) / pair.reserveB;
        require(amountAOptimal <= amountADesired && amountAOptimal >= amountAMin, "SimpleDEX: insufficient A amount");
        return (amountAOptimal, amountBDesired);
    }
    
    function _sqrt(uint256 y) internal pure returns (uint256 z) {
        if (y > 3) {
            z = y;
            uint256 x = y / 2 + 1;
            while (x < z) {
                z = x;
                x = (y / x + x) / 2;
            }
        } else if (y != 0) {
            z = 1;
        }
    }
}