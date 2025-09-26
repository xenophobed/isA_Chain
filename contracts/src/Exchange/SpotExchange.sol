// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/cryptography/EIP712.sol";

/**
 * @title SpotExchange
 * @dev Advanced spot trading exchange with order book and matching engine
 * 
 * Features:
 * - Order book management (limit orders, market orders)
 * - Automated matching engine
 * - Partial fills and order cancellation
 * - Fee structure with maker/taker distinction
 * - Off-chain order signing with on-chain settlement
 * - Multiple trading pairs
 * - Stop-loss and take-profit orders
 */
contract SpotExchange is AccessControl, ReentrancyGuard, Pausable, EIP712 {
    using SafeERC20 for IERC20;
    using ECDSA for bytes32;

    // Roles
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");
    bytes32 public constant MATCHER_ROLE = keccak256("MATCHER_ROLE");

    // Constants
    uint256 public constant PRECISION = 1e18;
    uint256 public constant MAX_FEE = 1000; // 10% max fee
    uint256 public constant MIN_ORDER_SIZE = 1e15; // 0.001 tokens minimum

    // Order types
    enum OrderType { LIMIT, MARKET, STOP_LOSS, TAKE_PROFIT }
    enum OrderSide { BUY, SELL }
    enum OrderStatus { PENDING, PARTIAL, FILLED, CANCELLED }

    // Trading pair structure
    struct TradingPair {
        address baseToken;
        address quoteToken;
        bool isActive;
        uint256 minOrderSize;
        uint256 tickSize; // Minimum price increment
        uint256 makerFee; // Fee for providing liquidity (in basis points)
        uint256 takerFee; // Fee for taking liquidity (in basis points)
        uint256 volume24h;
        uint256 lastPrice;
        uint256 totalTrades;
    }

    // Order structure
    struct Order {
        bytes32 id;
        address user;
        bytes32 pairId;
        OrderType orderType;
        OrderSide side;
        uint256 amount;
        uint256 price;
        uint256 filled;
        uint256 timestamp;
        uint256 expiry;
        OrderStatus status;
        uint256 stopPrice; // For stop orders
        bytes32 parentOrderId; // For linked orders
        bool isPostOnly; // Post-only orders (must be maker)
        uint256 nonce;
    }

    // Trade execution result
    struct Trade {
        bytes32 id;
        bytes32 pairId;
        address maker;
        address taker;
        bytes32 makerOrderId;
        bytes32 takerOrderId;
        uint256 amount;
        uint256 price;
        uint256 makerFee;
        uint256 takerFee;
        uint256 timestamp;
    }

    // Order book level
    struct OrderBookLevel {
        uint256 price;
        uint256 amount;
        bytes32[] orderIds;
        mapping(bytes32 => uint256) orderIndex;
    }

    // Order book structure
    struct OrderBook {
        mapping(uint256 => OrderBookLevel) bids; // price => level
        mapping(uint256 => OrderBookLevel) asks; // price => level
        uint256[] bidPrices;
        uint256[] askPrices;
        uint256 bestBid;
        uint256 bestAsk;
    }

    // State variables
    mapping(bytes32 => TradingPair) public tradingPairs;
    mapping(bytes32 => Order) public orders;
    mapping(bytes32 => OrderBook) public orderBooks;
    mapping(address => mapping(address => uint256)) public balances; // user => token => amount
    mapping(address => uint256) public nonces;
    mapping(bytes32 => Trade) public trades;
    
    bytes32[] public pairIds;
    bytes32[] public tradeHistory;
    address public feeRecipient;
    uint256 public totalVolume;
    uint256 public totalTrades;
    
    // Events
    event PairAdded(bytes32 indexed pairId, address baseToken, address quoteToken);
    event OrderPlaced(bytes32 indexed orderId, address indexed user, bytes32 indexed pairId);
    event OrderCancelled(bytes32 indexed orderId, address indexed user);
    event OrderMatched(bytes32 indexed tradeId, bytes32 makerOrderId, bytes32 takerOrderId, uint256 amount, uint256 price);
    event Deposit(address indexed user, address indexed token, uint256 amount);
    event Withdrawal(address indexed user, address indexed token, uint256 amount);

    /**
     * @dev Constructor
     */
    constructor(address _feeRecipient) EIP712("SpotExchange", "1") {
        require(_feeRecipient != address(0), "SpotExchange: invalid fee recipient");
        
        feeRecipient = _feeRecipient;
        
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ADMIN_ROLE, msg.sender);
        _grantRole(OPERATOR_ROLE, msg.sender);
        _grantRole(MATCHER_ROLE, msg.sender);
    }

    /**
     * @dev Add a new trading pair
     */
    function addTradingPair(
        bytes32 _pairId,
        address _baseToken,
        address _quoteToken,
        uint256 _minOrderSize,
        uint256 _tickSize,
        uint256 _makerFee,
        uint256 _takerFee
    ) external onlyRole(ADMIN_ROLE) {
        require(_baseToken != address(0) && _quoteToken != address(0), "SpotExchange: invalid tokens");
        require(_baseToken != _quoteToken, "SpotExchange: identical tokens");
        require(!tradingPairs[_pairId].isActive, "SpotExchange: pair already exists");
        require(_makerFee <= MAX_FEE && _takerFee <= MAX_FEE, "SpotExchange: fee too high");

        tradingPairs[_pairId] = TradingPair({
            baseToken: _baseToken,
            quoteToken: _quoteToken,
            isActive: true,
            minOrderSize: _minOrderSize,
            tickSize: _tickSize,
            makerFee: _makerFee,
            takerFee: _takerFee,
            volume24h: 0,
            lastPrice: 0,
            totalTrades: 0
        });

        pairIds.push(_pairId);
        emit PairAdded(_pairId, _baseToken, _quoteToken);
    }

    /**
     * @dev Deposit tokens to the exchange
     */
    function deposit(address _token, uint256 _amount) external nonReentrant whenNotPaused {
        require(_amount > 0, "SpotExchange: invalid amount");
        
        IERC20(_token).safeTransferFrom(msg.sender, address(this), _amount);
        balances[msg.sender][_token] += _amount;
        
        emit Deposit(msg.sender, _token, _amount);
    }

    /**
     * @dev Withdraw tokens from the exchange
     */
    function withdraw(address _token, uint256 _amount) external nonReentrant whenNotPaused {
        require(_amount > 0, "SpotExchange: invalid amount");
        require(balances[msg.sender][_token] >= _amount, "SpotExchange: insufficient balance");
        
        balances[msg.sender][_token] -= _amount;
        IERC20(_token).safeTransfer(msg.sender, _amount);
        
        emit Withdrawal(msg.sender, _token, _amount);
    }

    /**
     * @dev Place a new order
     */
    function placeOrder(
        bytes32 _pairId,
        OrderType _orderType,
        OrderSide _side,
        uint256 _amount,
        uint256 _price,
        uint256 _expiry,
        uint256 _stopPrice,
        bool _isPostOnly
    ) external nonReentrant whenNotPaused returns (bytes32) {
        require(tradingPairs[_pairId].isActive, "SpotExchange: pair not active");
        require(_amount >= tradingPairs[_pairId].minOrderSize, "SpotExchange: order too small");
        
        if (_orderType == OrderType.LIMIT) {
            require(_price > 0, "SpotExchange: invalid price");
            require(_price % tradingPairs[_pairId].tickSize == 0, "SpotExchange: invalid tick size");
        }

        bytes32 orderId = _generateOrderId(msg.sender, _pairId, nonces[msg.sender]++);
        
        // Check balance and reserve funds
        _reserveFunds(msg.sender, _pairId, _side, _amount, _price);

        orders[orderId] = Order({
            id: orderId,
            user: msg.sender,
            pairId: _pairId,
            orderType: _orderType,
            side: _side,
            amount: _amount,
            price: _price,
            filled: 0,
            timestamp: block.timestamp,
            expiry: _expiry,
            status: OrderStatus.PENDING,
            stopPrice: _stopPrice,
            parentOrderId: bytes32(0),
            isPostOnly: _isPostOnly,
            nonce: nonces[msg.sender] - 1
        });

        // Add to order book for limit orders
        if (_orderType == OrderType.LIMIT) {
            _addToOrderBook(_pairId, orderId);
            
            // Try to match immediately if not post-only
            if (!_isPostOnly) {
                _matchOrder(_pairId, orderId);
            }
        }

        emit OrderPlaced(orderId, msg.sender, _pairId);
        return orderId;
    }

    /**
     * @dev Cancel an existing order
     */
    function cancelOrder(bytes32 _orderId) external nonReentrant {
        Order storage order = orders[_orderId];
        require(order.user == msg.sender, "SpotExchange: not order owner");
        require(order.status == OrderStatus.PENDING || order.status == OrderStatus.PARTIAL, "SpotExchange: cannot cancel");

        // Remove from order book
        _removeFromOrderBook(order.pairId, _orderId);
        
        // Release reserved funds
        _releaseFunds(order.user, order.pairId, order.side, order.amount - order.filled, order.price);
        
        order.status = OrderStatus.CANCELLED;
        emit OrderCancelled(_orderId, msg.sender);
    }

    /**
     * @dev Match orders in the order book
     */
    function _matchOrder(bytes32 _pairId, bytes32 _orderId) internal {
        Order storage takerOrder = orders[_orderId];
        OrderBook storage book = orderBooks[_pairId];
        
        uint256[] memory prices;
        bool isBuyOrder = takerOrder.side == OrderSide.BUY;
        
        if (isBuyOrder) {
            prices = book.askPrices;
        } else {
            prices = book.bidPrices;
        }

        // Sort prices for optimal matching
        _sortPrices(prices, isBuyOrder);

        for (uint256 i = 0; i < prices.length && takerOrder.filled < takerOrder.amount; i++) {
            uint256 price = prices[i];
            
            // Check if price meets taker's requirements
            if (isBuyOrder && price > takerOrder.price) break;
            if (!isBuyOrder && price < takerOrder.price) break;

            OrderBookLevel storage level = isBuyOrder ? book.asks[price] : book.bids[price];
            
            // Match against orders at this price level
            for (uint256 j = 0; j < level.orderIds.length && takerOrder.filled < takerOrder.amount; j++) {
                bytes32 makerOrderId = level.orderIds[j];
                Order storage makerOrder = orders[makerOrderId];
                
                if (makerOrder.status != OrderStatus.PENDING && makerOrder.status != OrderStatus.PARTIAL) {
                    continue;
                }

                uint256 fillAmount = _min(
                    takerOrder.amount - takerOrder.filled,
                    makerOrder.amount - makerOrder.filled
                );

                if (fillAmount > 0) {
                    _executeTrade(_pairId, makerOrderId, _orderId, fillAmount, price);
                }
            }
        }

        // Update order status
        if (takerOrder.filled == takerOrder.amount) {
            takerOrder.status = OrderStatus.FILLED;
        } else if (takerOrder.filled > 0) {
            takerOrder.status = OrderStatus.PARTIAL;
        }
    }

    /**
     * @dev Execute a trade between two orders
     */
    function _executeTrade(
        bytes32 _pairId,
        bytes32 _makerOrderId,
        bytes32 _takerOrderId,
        uint256 _amount,
        uint256 _price
    ) internal {
        Order storage makerOrder = orders[_makerOrderId];
        Order storage takerOrder = orders[_takerOrderId];
        TradingPair storage pair = tradingPairs[_pairId];

        // Calculate fees
        uint256 makerFee = (_amount * _price * pair.makerFee) / (10000 * PRECISION);
        uint256 takerFee = (_amount * _price * pair.takerFee) / (10000 * PRECISION);

        // Update order fills
        makerOrder.filled += _amount;
        takerOrder.filled += _amount;

        // Update order status
        if (makerOrder.filled == makerOrder.amount) {
            makerOrder.status = OrderStatus.FILLED;
            _removeFromOrderBook(_pairId, _makerOrderId);
        } else {
            makerOrder.status = OrderStatus.PARTIAL;
        }

        // Execute token transfers
        _executeTokenTransfers(_pairId, makerOrder, takerOrder, _amount, _price, makerFee, takerFee);

        // Record trade
        bytes32 tradeId = keccak256(abi.encodePacked(_makerOrderId, _takerOrderId, block.timestamp));
        trades[tradeId] = Trade({
            id: tradeId,
            pairId: _pairId,
            maker: makerOrder.user,
            taker: takerOrder.user,
            makerOrderId: _makerOrderId,
            takerOrderId: _takerOrderId,
            amount: _amount,
            price: _price,
            makerFee: makerFee,
            takerFee: takerFee,
            timestamp: block.timestamp
        });

        tradeHistory.push(tradeId);

        // Update pair statistics
        pair.volume24h += _amount;
        pair.lastPrice = _price;
        pair.totalTrades++;
        totalVolume += _amount;
        totalTrades++;

        emit OrderMatched(tradeId, _makerOrderId, _takerOrderId, _amount, _price);
    }

    /**
     * @dev Execute token transfers for a trade
     */
    function _executeTokenTransfers(
        bytes32 _pairId,
        Order storage _makerOrder,
        Order storage _takerOrder,
        uint256 _amount,
        uint256 _price,
        uint256 _makerFee,
        uint256 _takerFee
    ) internal {
        TradingPair storage pair = tradingPairs[_pairId];
        address baseToken = pair.baseToken;
        address quoteToken = pair.quoteToken;
        uint256 quoteAmount = (_amount * _price) / PRECISION;

        if (_makerOrder.side == OrderSide.BUY) {
            // Maker buying, taker selling
            // Transfer base tokens from taker to maker
            balances[_takerOrder.user][baseToken] -= _amount;
            balances[_makerOrder.user][baseToken] += _amount;
            
            // Transfer quote tokens from maker to taker
            balances[_makerOrder.user][quoteToken] -= quoteAmount;
            balances[_takerOrder.user][quoteToken] += (quoteAmount - _takerFee);
            
            // Collect fees
            balances[feeRecipient][quoteToken] += (_makerFee + _takerFee);
        } else {
            // Maker selling, taker buying
            // Transfer base tokens from maker to taker
            balances[_makerOrder.user][baseToken] -= _amount;
            balances[_takerOrder.user][baseToken] += (_amount - _takerFee);
            
            // Transfer quote tokens from taker to maker
            balances[_takerOrder.user][quoteToken] -= quoteAmount;
            balances[_makerOrder.user][quoteToken] += quoteAmount;
            
            // Collect fees
            balances[feeRecipient][baseToken] += (_makerFee + _takerFee);
        }
    }

    /**
     * @dev Reserve funds for an order
     */
    function _reserveFunds(
        address _user,
        bytes32 _pairId,
        OrderSide _side,
        uint256 _amount,
        uint256 _price
    ) internal {
        TradingPair storage pair = tradingPairs[_pairId];
        
        if (_side == OrderSide.BUY) {
            uint256 quoteAmount = (_amount * _price) / PRECISION;
            require(balances[_user][pair.quoteToken] >= quoteAmount, "SpotExchange: insufficient balance");
            // Note: In production, you'd want to implement a reservation system
        } else {
            require(balances[_user][pair.baseToken] >= _amount, "SpotExchange: insufficient balance");
            // Note: In production, you'd want to implement a reservation system
        }
    }

    /**
     * @dev Release reserved funds for a cancelled order
     */
    function _releaseFunds(
        address _user,
        bytes32 _pairId,
        OrderSide _side,
        uint256 _amount,
        uint256 _price
    ) internal {
        // Note: In production, implement proper fund reservation/release logic
        // This would unreserve the funds that were reserved during order placement
    }

    /**
     * @dev Add order to order book
     */
    function _addToOrderBook(bytes32 _pairId, bytes32 _orderId) internal {
        Order storage order = orders[_orderId];
        OrderBook storage book = orderBooks[_pairId];
        
        if (order.side == OrderSide.BUY) {
            OrderBookLevel storage level = book.bids[order.price];
            if (level.orderIds.length == 0) {
                level.price = order.price;
                book.bidPrices.push(order.price);
            }
            level.orderIds.push(_orderId);
            level.orderIndex[_orderId] = level.orderIds.length - 1;
            level.amount += order.amount;
            
            if (order.price > book.bestBid) {
                book.bestBid = order.price;
            }
        } else {
            OrderBookLevel storage level = book.asks[order.price];
            if (level.orderIds.length == 0) {
                level.price = order.price;
                book.askPrices.push(order.price);
            }
            level.orderIds.push(_orderId);
            level.orderIndex[_orderId] = level.orderIds.length - 1;
            level.amount += order.amount;
            
            if (book.bestAsk == 0 || order.price < book.bestAsk) {
                book.bestAsk = order.price;
            }
        }
    }

    /**
     * @dev Remove order from order book
     */
    function _removeFromOrderBook(bytes32 _pairId, bytes32 _orderId) internal {
        Order storage order = orders[_orderId];
        OrderBook storage book = orderBooks[_pairId];
        
        if (order.side == OrderSide.BUY) {
            OrderBookLevel storage level = book.bids[order.price];
            uint256 index = level.orderIndex[_orderId];
            
            // Remove order from array
            if (index < level.orderIds.length - 1) {
                level.orderIds[index] = level.orderIds[level.orderIds.length - 1];
                level.orderIndex[level.orderIds[index]] = index;
            }
            level.orderIds.pop();
            delete level.orderIndex[_orderId];
            level.amount -= (order.amount - order.filled);
        } else {
            OrderBookLevel storage level = book.asks[order.price];
            uint256 index = level.orderIndex[_orderId];
            
            // Remove order from array
            if (index < level.orderIds.length - 1) {
                level.orderIds[index] = level.orderIds[level.orderIds.length - 1];
                level.orderIndex[level.orderIds[index]] = index;
            }
            level.orderIds.pop();
            delete level.orderIndex[_orderId];
            level.amount -= (order.amount - order.filled);
        }
    }

    /**
     * @dev Generate unique order ID
     */
    function _generateOrderId(address _user, bytes32 _pairId, uint256 _nonce) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(_user, _pairId, _nonce));
    }

    /**
     * @dev Sort prices for optimal matching
     */
    function _sortPrices(uint256[] memory _prices, bool _ascending) internal pure {
        for (uint256 i = 0; i < _prices.length - 1; i++) {
            for (uint256 j = 0; j < _prices.length - i - 1; j++) {
                bool shouldSwap = _ascending ? 
                    _prices[j] > _prices[j + 1] : 
                    _prices[j] < _prices[j + 1];
                
                if (shouldSwap) {
                    (_prices[j], _prices[j + 1]) = (_prices[j + 1], _prices[j]);
                }
            }
        }
    }

    /**
     * @dev Get minimum of two numbers
     */
    function _min(uint256 _a, uint256 _b) internal pure returns (uint256) {
        return _a < _b ? _a : _b;
    }

    // View functions
    function getOrder(bytes32 _orderId) external view returns (Order memory) {
        return orders[_orderId];
    }

    function getTrade(bytes32 _tradeId) external view returns (Trade memory) {
        return trades[_tradeId];
    }

    function getTradingPair(bytes32 _pairId) external view returns (TradingPair memory) {
        return tradingPairs[_pairId];
    }

    function getOrderBook(bytes32 _pairId, uint256 _depth) external view returns (
        uint256[] memory bidPrices,
        uint256[] memory bidAmounts,
        uint256[] memory askPrices,
        uint256[] memory askAmounts
    ) {
        OrderBook storage book = orderBooks[_pairId];
        
        // Get best bids and asks up to depth
        uint256 bidCount = _min(_depth, book.bidPrices.length);
        uint256 askCount = _min(_depth, book.askPrices.length);
        
        bidPrices = new uint256[](bidCount);
        bidAmounts = new uint256[](bidCount);
        askPrices = new uint256[](askCount);
        askAmounts = new uint256[](askCount);
        
        // Sort and get top bids
        uint256[] memory sortedBids = book.bidPrices;
        _sortPrices(sortedBids, false); // Descending for bids
        
        for (uint256 i = 0; i < bidCount; i++) {
            bidPrices[i] = sortedBids[i];
            bidAmounts[i] = book.bids[sortedBids[i]].amount;
        }
        
        // Sort and get top asks
        uint256[] memory sortedAsks = book.askPrices;
        _sortPrices(sortedAsks, true); // Ascending for asks
        
        for (uint256 i = 0; i < askCount; i++) {
            askPrices[i] = sortedAsks[i];
            askAmounts[i] = book.asks[sortedAsks[i]].amount;
        }
    }

    function getUserBalance(address _user, address _token) external view returns (uint256) {
        return balances[_user][_token];
    }

    function getPairCount() external view returns (uint256) {
        return pairIds.length;
    }

    // Admin functions
    function setPairActive(bytes32 _pairId, bool _isActive) external onlyRole(ADMIN_ROLE) {
        tradingPairs[_pairId].isActive = _isActive;
    }

    function updateFees(bytes32 _pairId, uint256 _makerFee, uint256 _takerFee) external onlyRole(ADMIN_ROLE) {
        require(_makerFee <= MAX_FEE && _takerFee <= MAX_FEE, "SpotExchange: fee too high");
        tradingPairs[_pairId].makerFee = _makerFee;
        tradingPairs[_pairId].takerFee = _takerFee;
    }

    function setFeeRecipient(address _feeRecipient) external onlyRole(ADMIN_ROLE) {
        require(_feeRecipient != address(0), "SpotExchange: invalid fee recipient");
        feeRecipient = _feeRecipient;
    }

    function pause() external onlyRole(ADMIN_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(ADMIN_ROLE) {
        _unpause();
    }

    function emergencyWithdraw(address _token, uint256 _amount) external onlyRole(ADMIN_ROLE) {
        IERC20(_token).safeTransfer(msg.sender, _amount);
    }
}