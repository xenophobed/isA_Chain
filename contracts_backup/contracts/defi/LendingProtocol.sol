// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/math/Math.sol";

/**
 * @title LendingProtocol
 * @dev Advanced lending/borrowing protocol for the isA_Chain ecosystem
 * 
 * Features:
 * - Multi-asset lending and borrowing
 * - Dynamic interest rates based on utilization
 * - Collateral-based lending with liquidation
 * - Flash loans
 * - Compound interest calculation
 * - Risk management and health factors
 */
contract LendingProtocol is AccessControl, ReentrancyGuard, Pausable {
    using SafeERC20 for IERC20;
    using Math for uint256;

    // Roles
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant ORACLE_ROLE = keccak256("ORACLE_ROLE");
    bytes32 public constant LIQUIDATOR_ROLE = keccak256("LIQUIDATOR_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    // Constants
    uint256 public constant PRECISION = 1e18;
    uint256 public constant LIQUIDATION_THRESHOLD = 8000; // 80% in basis points
    uint256 public constant LIQUIDATION_BONUS = 500; // 5% bonus for liquidators
    uint256 public constant MAX_BORROW_RATE = 1000; // 10% per block maximum
    uint256 public constant FLASH_LOAN_FEE = 9; // 0.09%
    uint256 public constant SECONDS_PER_YEAR = 31536000;

    // Market configuration
    struct Market {
        bool isActive;
        bool canBorrow;
        bool canUseAsCollateral;
        address underlying;
        uint256 totalSupply;
        uint256 totalBorrows;
        uint256 totalReserves;
        uint256 reserveFactor; // Percentage of interest that goes to reserves
        uint256 collateralFactor; // Max borrow power as percentage of collateral value
        uint256 liquidationIncentive; // Additional collateral given to liquidators
        uint256 baseRatePerYear;
        uint256 multiplierPerYear;
        uint256 jumpMultiplierPerYear;
        uint256 kink; // Utilization point at which jump multiplier is applied
        uint256 lastUpdateTimestamp;
        uint256 borrowIndex;
        uint256 supplyIndex;
    }

    // User account information
    struct AccountLiquidity {
        uint256 collateralValueUSD;
        uint256 borrowValueUSD;
        uint256 healthFactor;
    }

    // Borrow/Supply balances
    struct Balance {
        uint256 principal;
        uint256 interestIndex;
    }

    // Flash loan data
    struct FlashLoanData {
        address asset;
        uint256 amount;
        uint256 fee;
        address receiver;
        bytes params;
    }

    // State variables
    mapping(address => Market) public markets;
    mapping(address => mapping(address => Balance)) public supplyBalances; // user => asset => balance
    mapping(address => mapping(address => Balance)) public borrowBalances; // user => asset => balance
    mapping(address => address[]) public userAssets; // user => list of assets they interact with
    mapping(address => uint256) public assetPrices; // asset => price in USD (with 18 decimals)
    
    address[] public allMarkets;
    address public treasury;
    uint256 public flashLoanFeesCollected;
    
    // Events
    event MarketAdded(address indexed asset, address indexed underlying);
    event Supply(address indexed user, address indexed asset, uint256 amount, uint256 balance);
    event Withdraw(address indexed user, address indexed asset, uint256 amount, uint256 balance);
    event Borrow(address indexed user, address indexed asset, uint256 amount, uint256 balance);
    event RepayBorrow(address indexed user, address indexed asset, uint256 amount, uint256 balance);
    event Liquidation(
        address indexed liquidator,
        address indexed borrower,
        address indexed assetBorrowed,
        address assetCollateral,
        uint256 repayAmount,
        uint256 seizeAmount
    );
    event FlashLoan(
        address indexed receiver,
        address indexed asset,
        uint256 amount,
        uint256 fee
    );
    event PriceUpdated(address indexed asset, uint256 newPrice);
    event MarketConfigUpdated(address indexed asset);

    /**
     * @dev Constructor
     * @param _treasury Treasury address for reserves
     */
    constructor(address _treasury) {
        require(_treasury != address(0), "LendingProtocol: invalid treasury");
        
        treasury = _treasury;
        
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ADMIN_ROLE, msg.sender);
        _grantRole(ORACLE_ROLE, msg.sender);
        _grantRole(LIQUIDATOR_ROLE, msg.sender);
        _grantRole(PAUSER_ROLE, msg.sender);
    }

    /**
     * @dev Add a new lending market
     */
    function addMarket(
        address asset,
        address underlying,
        uint256 collateralFactor,
        uint256 reserveFactor,
        uint256 baseRatePerYear,
        uint256 multiplierPerYear,
        uint256 jumpMultiplierPerYear,
        uint256 kink
    ) external onlyRole(ADMIN_ROLE) {
        require(asset != address(0) && underlying != address(0), "LendingProtocol: invalid addresses");
        require(!markets[asset].isActive, "LendingProtocol: market already exists");
        require(collateralFactor <= 9000, "LendingProtocol: collateral factor too high"); // Max 90%
        require(reserveFactor <= 5000, "LendingProtocol: reserve factor too high"); // Max 50%

        markets[asset] = Market({
            isActive: true,
            canBorrow: true,
            canUseAsCollateral: true,
            underlying: underlying,
            totalSupply: 0,
            totalBorrows: 0,
            totalReserves: 0,
            reserveFactor: reserveFactor,
            collateralFactor: collateralFactor,
            liquidationIncentive: LIQUIDATION_BONUS,
            baseRatePerYear: baseRatePerYear,
            multiplierPerYear: multiplierPerYear,
            jumpMultiplierPerYear: jumpMultiplierPerYear,
            kink: kink,
            lastUpdateTimestamp: block.timestamp,
            borrowIndex: PRECISION,
            supplyIndex: PRECISION
        });

        allMarkets.push(asset);
        emit MarketAdded(asset, underlying);
    }

    /**
     * @dev Supply assets to earn interest
     */
    function supply(address asset, uint256 amount) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        require(markets[asset].isActive, "LendingProtocol: market not active");
        require(amount > 0, "LendingProtocol: invalid amount");

        Market storage market = markets[asset];
        _accrueInterest(asset);

        // Calculate supply tokens to mint
        uint256 supplyTokens = amount;
        if (market.totalSupply > 0) {
            supplyTokens = (amount * PRECISION) / market.supplyIndex;
        }

        // Update user balance
        Balance storage balance = supplyBalances[msg.sender][asset];
        balance.principal += supplyTokens;
        balance.interestIndex = market.supplyIndex;

        // Update market state
        market.totalSupply += supplyTokens;

        // Add to user assets if first time
        _addUserAsset(msg.sender, asset);

        // Transfer tokens
        IERC20(market.underlying).safeTransferFrom(msg.sender, address(this), amount);

        emit Supply(msg.sender, asset, amount, balance.principal);
    }

    /**
     * @dev Withdraw supplied assets
     */
    function withdraw(address asset, uint256 amount) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        require(markets[asset].isActive, "LendingProtocol: market not active");
        
        Market storage market = markets[asset];
        _accrueInterest(asset);

        Balance storage balance = supplyBalances[msg.sender][asset];
        uint256 currentSupply = (balance.principal * market.supplyIndex) / PRECISION;
        
        require(amount > 0 && amount <= currentSupply, "LendingProtocol: invalid amount");

        // Check if withdrawal would cause undercollateralization
        _checkAccountLiquidity(msg.sender, asset, amount, 0);

        // Calculate supply tokens to burn
        uint256 supplyTokens = (amount * PRECISION) / market.supplyIndex;

        // Update balances
        balance.principal -= supplyTokens;
        balance.interestIndex = market.supplyIndex;
        market.totalSupply -= supplyTokens;

        // Transfer tokens
        IERC20(market.underlying).safeTransfer(msg.sender, amount);

        emit Withdraw(msg.sender, asset, amount, balance.principal);
    }

    /**
     * @dev Borrow assets using collateral
     */
    function borrow(address asset, uint256 amount) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        require(markets[asset].isActive && markets[asset].canBorrow, "LendingProtocol: borrowing not allowed");
        require(amount > 0, "LendingProtocol: invalid amount");

        Market storage market = markets[asset];
        _accrueInterest(asset);

        // Check if user has sufficient collateral
        _checkAccountLiquidity(msg.sender, address(0), 0, amount);

        // Calculate borrow tokens
        uint256 borrowTokens = (amount * PRECISION) / market.borrowIndex;

        // Update user balance
        Balance storage balance = borrowBalances[msg.sender][asset];
        balance.principal += borrowTokens;
        balance.interestIndex = market.borrowIndex;

        // Update market state
        market.totalBorrows += borrowTokens;

        // Add to user assets if first time
        _addUserAsset(msg.sender, asset);

        // Transfer tokens
        IERC20(market.underlying).safeTransfer(msg.sender, amount);

        emit Borrow(msg.sender, asset, amount, balance.principal);
    }

    /**
     * @dev Repay borrowed assets
     */
    function repayBorrow(address asset, uint256 amount) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        require(markets[asset].isActive, "LendingProtocol: market not active");

        Market storage market = markets[asset];
        _accrueInterest(asset);

        Balance storage balance = borrowBalances[msg.sender][asset];
        uint256 currentBorrow = (balance.principal * market.borrowIndex) / PRECISION;
        
        // Allow repaying up to the full balance
        uint256 repayAmount = amount;
        if (amount > currentBorrow) {
            repayAmount = currentBorrow;
        }

        require(repayAmount > 0, "LendingProtocol: nothing to repay");

        // Calculate borrow tokens to burn
        uint256 borrowTokens = (repayAmount * PRECISION) / market.borrowIndex;

        // Update balances
        balance.principal -= borrowTokens;
        balance.interestIndex = market.borrowIndex;
        market.totalBorrows -= borrowTokens;

        // Transfer tokens
        IERC20(market.underlying).safeTransferFrom(msg.sender, address(this), repayAmount);

        emit RepayBorrow(msg.sender, asset, repayAmount, balance.principal);
    }

    /**
     * @dev Liquidate an undercollateralized borrow
     */
    function liquidate(
        address borrower,
        address assetBorrowed,
        uint256 repayAmount,
        address assetCollateral
    ) external onlyRole(LIQUIDATOR_ROLE) nonReentrant {
        require(borrower != msg.sender, "LendingProtocol: cannot liquidate self");
        require(markets[assetBorrowed].isActive && markets[assetCollateral].isActive, "LendingProtocol: inactive market");

        _accrueInterest(assetBorrowed);
        _accrueInterest(assetCollateral);

        // Check if borrower is actually undercollateralized
        AccountLiquidity memory liquidity = _getAccountLiquidity(borrower);
        require(liquidity.healthFactor < PRECISION, "LendingProtocol: borrower not undercollateralized");

        // Calculate maximum liquidation amount (50% of borrow)
        Balance storage borrowBalance = borrowBalances[borrower][assetBorrowed];
        uint256 maxLiquidation = (borrowBalance.principal * markets[assetBorrowed].borrowIndex) / (2 * PRECISION);
        require(repayAmount <= maxLiquidation, "LendingProtocol: liquidation amount too high");

        // Calculate collateral to seize
        uint256 seizeAmount = _calculateSeizeAmount(assetBorrowed, assetCollateral, repayAmount);

        // Update borrower's borrow balance
        uint256 borrowTokens = (repayAmount * PRECISION) / markets[assetBorrowed].borrowIndex;
        borrowBalance.principal -= borrowTokens;
        markets[assetBorrowed].totalBorrows -= borrowTokens;

        // Update borrower's collateral balance
        Balance storage collateralBalance = supplyBalances[borrower][assetCollateral];
        uint256 supplyTokens = (seizeAmount * PRECISION) / markets[assetCollateral].supplyIndex;
        collateralBalance.principal -= supplyTokens;
        markets[assetCollateral].totalSupply -= supplyTokens;

        // Transfer repay amount from liquidator
        IERC20(markets[assetBorrowed].underlying).safeTransferFrom(msg.sender, address(this), repayAmount);
        
        // Transfer seized collateral to liquidator
        IERC20(markets[assetCollateral].underlying).safeTransfer(msg.sender, seizeAmount);

        emit Liquidation(msg.sender, borrower, assetBorrowed, assetCollateral, repayAmount, seizeAmount);
    }

    /**
     * @dev Execute a flash loan
     */
    function flashLoan(
        address asset,
        uint256 amount,
        address receiver,
        bytes calldata params
    ) external nonReentrant whenNotPaused {
        require(markets[asset].isActive, "LendingProtocol: market not active");
        require(amount > 0, "LendingProtocol: invalid amount");

        uint256 fee = (amount * FLASH_LOAN_FEE) / 10000;
        uint256 balanceBefore = IERC20(markets[asset].underlying).balanceOf(address(this));

        // Transfer loan amount to receiver
        IERC20(markets[asset].underlying).safeTransfer(receiver, amount);

        // Call receiver's callback function
        IFlashLoanReceiver(receiver).executeOperation(asset, amount, fee, params);

        // Check repayment
        uint256 balanceAfter = IERC20(markets[asset].underlying).balanceOf(address(this));
        require(balanceAfter >= balanceBefore + fee, "LendingProtocol: flash loan not repaid");

        // Collect fees
        flashLoanFeesCollected += fee;

        emit FlashLoan(receiver, asset, amount, fee);
    }

    /**
     * @dev Update asset price (Oracle role)
     */
    function updatePrice(address asset, uint256 priceUSD) 
        external 
        onlyRole(ORACLE_ROLE) 
    {
        require(markets[asset].isActive, "LendingProtocol: market not active");
        assetPrices[asset] = priceUSD;
        emit PriceUpdated(asset, priceUSD);
    }

    /**
     * @dev Accrue interest for a market
     */
    function _accrueInterest(address asset) internal {
        Market storage market = markets[asset];
        uint256 currentTime = block.timestamp;
        
        if (market.lastUpdateTimestamp == currentTime) {
            return;
        }

        uint256 timeDelta = currentTime - market.lastUpdateTimestamp;
        uint256 borrowRate = _getBorrowRate(asset);
        uint256 interestAccumulated = (market.totalBorrows * borrowRate * timeDelta) / SECONDS_PER_YEAR;

        // Update borrow index
        market.borrowIndex += (market.borrowIndex * borrowRate * timeDelta) / SECONDS_PER_YEAR;

        // Calculate reserves
        uint256 totalReservesNew = interestAccumulated * market.reserveFactor / 10000;
        market.totalReserves += totalReservesNew;

        // Update supply index
        uint256 supplyInterest = interestAccumulated - totalReservesNew;
        if (market.totalSupply > 0) {
            market.supplyIndex += (market.supplyIndex * supplyInterest) / (market.totalSupply * PRECISION);
        }

        market.lastUpdateTimestamp = currentTime;
    }

    /**
     * @dev Calculate current borrow rate for a market
     */
    function _getBorrowRate(address asset) internal view returns (uint256) {
        Market storage market = markets[asset];
        
        if (market.totalSupply == 0) {
            return market.baseRatePerYear;
        }

        uint256 utilizationRate = (market.totalBorrows * PRECISION) / market.totalSupply;
        
        if (utilizationRate <= market.kink) {
            return market.baseRatePerYear + (utilizationRate * market.multiplierPerYear) / PRECISION;
        } else {
            uint256 normalRate = market.baseRatePerYear + (market.kink * market.multiplierPerYear) / PRECISION;
            uint256 excessUtilization = utilizationRate - market.kink;
            return normalRate + (excessUtilization * market.jumpMultiplierPerYear) / PRECISION;
        }
    }

    /**
     * @dev Check account liquidity and revert if undercollateralized
     */
    function _checkAccountLiquidity(
        address user, 
        address withdrawAsset, 
        uint256 withdrawAmount, 
        uint256 borrowAmount
    ) internal view {
        AccountLiquidity memory liquidity = _calculateAccountLiquidity(user, withdrawAsset, withdrawAmount, borrowAmount);
        require(liquidity.healthFactor >= PRECISION, "LendingProtocol: insufficient collateral");
    }

    /**
     * @dev Calculate account liquidity considering hypothetical changes
     */
    function _calculateAccountLiquidity(
        address user,
        address withdrawAsset,
        uint256 withdrawAmount,
        uint256 borrowAmount
    ) internal view returns (AccountLiquidity memory) {
        uint256 collateralValue = 0;
        uint256 borrowValue = 0;

        address[] memory assets = userAssets[user];
        
        for (uint256 i = 0; i < assets.length; i++) {
            address asset = assets[i];
            Market storage market = markets[asset];
            uint256 price = assetPrices[asset];

            // Calculate supply value
            Balance storage supplyBalance = supplyBalances[user][asset];
            if (supplyBalance.principal > 0 && market.canUseAsCollateral) {
                uint256 supplyValue = (supplyBalance.principal * market.supplyIndex * price) / (PRECISION * PRECISION);
                
                // Subtract hypothetical withdrawal
                if (asset == withdrawAsset) {
                    uint256 withdrawValue = (withdrawAmount * price) / PRECISION;
                    supplyValue = supplyValue > withdrawValue ? supplyValue - withdrawValue : 0;
                }
                
                collateralValue += (supplyValue * market.collateralFactor) / 10000;
            }

            // Calculate borrow value
            Balance storage borrowBalance = borrowBalances[user][asset];
            if (borrowBalance.principal > 0) {
                uint256 currentBorrowValue = (borrowBalance.principal * market.borrowIndex * price) / (PRECISION * PRECISION);
                
                // Add hypothetical borrow
                if (asset == address(0)) { // Using address(0) to represent any asset for new borrow
                    currentBorrowValue += (borrowAmount * price) / PRECISION;
                }
                
                borrowValue += currentBorrowValue;
            }
        }

        uint256 healthFactor = borrowValue > 0 ? (collateralValue * PRECISION) / borrowValue : type(uint256).max;

        return AccountLiquidity({
            collateralValueUSD: collateralValue,
            borrowValueUSD: borrowValue,
            healthFactor: healthFactor
        });
    }

    /**
     * @dev Get current account liquidity
     */
    function _getAccountLiquidity(address user) internal view returns (AccountLiquidity memory) {
        return _calculateAccountLiquidity(user, address(0), 0, 0);
    }

    /**
     * @dev Calculate amount of collateral to seize during liquidation
     */
    function _calculateSeizeAmount(
        address assetBorrowed,
        address assetCollateral,
        uint256 repayAmount
    ) internal view returns (uint256) {
        uint256 priceBorrowed = assetPrices[assetBorrowed];
        uint256 priceCollateral = assetPrices[assetCollateral];
        
        require(priceBorrowed > 0 && priceCollateral > 0, "LendingProtocol: invalid prices");

        // Calculate collateral amount with liquidation incentive
        uint256 seizeAmountUSD = (repayAmount * priceBorrowed * (10000 + markets[assetCollateral].liquidationIncentive)) / 10000;
        return (seizeAmountUSD * PRECISION) / priceCollateral;
    }

    /**
     * @dev Add asset to user's asset list
     */
    function _addUserAsset(address user, address asset) internal {
        address[] storage assets = userAssets[user];
        for (uint256 i = 0; i < assets.length; i++) {
            if (assets[i] == asset) {
                return; // Already exists
            }
        }
        assets.push(asset);
    }

    // View functions
    function getAccountLiquidity(address user) external view returns (AccountLiquidity memory) {
        return _getAccountLiquidity(user);
    }

    function getBorrowRate(address asset) external view returns (uint256) {
        return _getBorrowRate(asset);
    }

    function getSupplyRate(address asset) external view returns (uint256) {
        uint256 borrowRate = _getBorrowRate(asset);
        Market storage market = markets[asset];
        
        if (market.totalSupply == 0) {
            return 0;
        }
        
        uint256 utilizationRate = (market.totalBorrows * PRECISION) / market.totalSupply;
        return (utilizationRate * borrowRate * (10000 - market.reserveFactor)) / (10000 * PRECISION);
    }

    function getUserSupplyBalance(address user, address asset) external view returns (uint256) {
        Balance storage balance = supplyBalances[user][asset];
        return (balance.principal * markets[asset].supplyIndex) / PRECISION;
    }

    function getUserBorrowBalance(address user, address asset) external view returns (uint256) {
        Balance storage balance = borrowBalances[user][asset];
        return (balance.principal * markets[asset].borrowIndex) / PRECISION;
    }

    // Admin functions
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function withdrawReserves(address asset, uint256 amount) 
        external 
        onlyRole(ADMIN_ROLE) 
    {
        Market storage market = markets[asset];
        require(amount <= market.totalReserves, "LendingProtocol: insufficient reserves");
        
        market.totalReserves -= amount;
        IERC20(market.underlying).safeTransfer(treasury, amount);
    }

    function withdrawFlashLoanFees() external onlyRole(ADMIN_ROLE) {
        require(flashLoanFeesCollected > 0, "LendingProtocol: no fees to withdraw");
        
        uint256 amount = flashLoanFeesCollected;
        flashLoanFeesCollected = 0;
        
        // Transfer fees to treasury (assuming collected in native token)
        payable(treasury).transfer(amount);
    }
}

/**
 * @title IFlashLoanReceiver
 * @dev Interface for flash loan receivers
 */
interface IFlashLoanReceiver {
    function executeOperation(
        address asset,
        uint256 amount,
        uint256 fee,
        bytes calldata params
    ) external;
}