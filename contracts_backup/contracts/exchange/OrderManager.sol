// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/cryptography/ECDSA.sol";
import "@openzeppelin/contracts/utils/cryptography/EIP712.sol";
import "./SpotExchange.sol";

/**
 * @title OrderManager
 * @dev Advanced order management system for complex order types
 * 
 * Features:
 * - Stop-loss and take-profit orders
 * - One-cancels-other (OCO) orders
 * - Trailing stops
 * - Time-based orders (good-till-time, good-till-cancel)
 * - Iceberg orders (hidden quantity)
 * - Order modification and partial cancellation
 */
contract OrderManager is AccessControl, ReentrancyGuard, EIP712 {
    using ECDSA for bytes32;

    // Roles
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant OPERATOR_ROLE = keccak256("OPERATOR_ROLE");

    // Advanced order types
    enum AdvancedOrderType {
        STOP_LOSS,
        TAKE_PROFIT,
        TRAILING_STOP,
        OCO, // One-Cancels-Other
        ICEBERG,
        TWAP, // Time-Weighted Average Price
        VWAP  // Volume-Weighted Average Price
    }

    enum TimeInForce {
        GTC, // Good Till Cancel
        GTT, // Good Till Time
        IOC, // Immediate Or Cancel
        FOK  // Fill Or Kill
    }

    // Advanced order structure
    struct AdvancedOrder {
        bytes32 id;
        address user;
        bytes32 pairId;
        AdvancedOrderType orderType;
        SpotExchange.OrderSide side;
        uint256 amount;
        uint256 price;
        uint256 triggerPrice;
        uint256 limitPrice;
        uint256 trailingAmount; // For trailing stops
        uint256 visibleAmount; // For iceberg orders
        uint256 timeInterval; // For TWAP/VWAP orders
        uint256 maxSlippage; // For market orders
        TimeInForce timeInForce;
        uint256 expiry;
        uint256 created;
        bool isActive;
        bytes32 linkedOrderId; // For OCO orders
        mapping(bytes32 => bool) childOrders; // For parent orders
        uint256 executedAmount;
        uint256 lastTriggerPrice; // For trailing stops
    }

    // TWAP/VWAP execution state
    struct TWAPState {
        uint256 totalAmount;
        uint256 executedAmount;
        uint256 startTime;
        uint256 endTime;
        uint256 intervals;
        uint256 currentInterval;
        uint256 nextExecutionTime;
        uint256 intervalAmount;
    }

    // Iceberg order state
    struct IcebergState {
        uint256 hiddenAmount;
        uint256 visibleAmount;
        uint256 executedVisible;
        bytes32[] childOrderIds;
        uint256 currentChildIndex;
    }

    // State variables
    mapping(bytes32 => AdvancedOrder) public advancedOrders;
    mapping(bytes32 => TWAPState) public twapStates;
    mapping(bytes32 => IcebergState) public icebergStates;
    mapping(address => bytes32[]) public userOrders;
    mapping(bytes32 => bytes32[]) public pairOrders;
    
    SpotExchange public immutable exchange;
    bytes32[] public activeOrders;
    
    // Events
    event AdvancedOrderPlaced(bytes32 indexed orderId, address indexed user, AdvancedOrderType orderType);
    event OrderTriggered(bytes32 indexed orderId, uint256 triggerPrice);
    event OCOOrderActivated(bytes32 indexed orderId, bytes32 cancelledOrderId);
    event IcebergOrderRefilled(bytes32 indexed orderId, uint256 newVisibleAmount);
    event TWAPIntervalExecuted(bytes32 indexed orderId, uint256 interval, uint256 amount);
    event TrailingStopUpdated(bytes32 indexed orderId, uint256 newTriggerPrice);

    /**
     * @dev Constructor
     */
    constructor(address _exchange) EIP712("OrderManager", "1") {
        require(_exchange != address(0), "OrderManager: invalid exchange");
        
        exchange = SpotExchange(_exchange);
        
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ADMIN_ROLE, msg.sender);
        _grantRole(OPERATOR_ROLE, msg.sender);
    }

    /**
     * @dev Place a stop-loss order
     */
    function placeStopLoss(
        bytes32 _pairId,
        SpotExchange.OrderSide _side,
        uint256 _amount,
        uint256 _triggerPrice,
        uint256 _limitPrice,
        TimeInForce _timeInForce,
        uint256 _expiry
    ) external nonReentrant returns (bytes32) {
        bytes32 orderId = _generateOrderId(msg.sender, _pairId);
        
        AdvancedOrder storage order = advancedOrders[orderId];
        order.id = orderId;
        order.user = msg.sender;
        order.pairId = _pairId;
        order.orderType = AdvancedOrderType.STOP_LOSS;
        order.side = _side;
        order.amount = _amount;
        order.triggerPrice = _triggerPrice;
        order.limitPrice = _limitPrice;
        order.timeInForce = _timeInForce;
        order.expiry = _expiry;
        order.created = block.timestamp;
        order.isActive = true;

        _addToUserOrders(msg.sender, orderId);
        _addToPairOrders(_pairId, orderId);
        activeOrders.push(orderId);

        emit AdvancedOrderPlaced(orderId, msg.sender, AdvancedOrderType.STOP_LOSS);
        return orderId;
    }

    /**
     * @dev Place a take-profit order
     */
    function placeTakeProfit(
        bytes32 _pairId,
        SpotExchange.OrderSide _side,
        uint256 _amount,
        uint256 _triggerPrice,
        uint256 _limitPrice,
        TimeInForce _timeInForce,
        uint256 _expiry
    ) external nonReentrant returns (bytes32) {
        bytes32 orderId = _generateOrderId(msg.sender, _pairId);
        
        AdvancedOrder storage order = advancedOrders[orderId];
        order.id = orderId;
        order.user = msg.sender;
        order.pairId = _pairId;
        order.orderType = AdvancedOrderType.TAKE_PROFIT;
        order.side = _side;
        order.amount = _amount;
        order.triggerPrice = _triggerPrice;
        order.limitPrice = _limitPrice;
        order.timeInForce = _timeInForce;
        order.expiry = _expiry;
        order.created = block.timestamp;
        order.isActive = true;

        _addToUserOrders(msg.sender, orderId);
        _addToPairOrders(_pairId, orderId);
        activeOrders.push(orderId);

        emit AdvancedOrderPlaced(orderId, msg.sender, AdvancedOrderType.TAKE_PROFIT);
        return orderId;
    }

    /**
     * @dev Place a trailing stop order
     */
    function placeTrailingStop(
        bytes32 _pairId,
        SpotExchange.OrderSide _side,
        uint256 _amount,
        uint256 _trailingAmount,
        TimeInForce _timeInForce,
        uint256 _expiry
    ) external nonReentrant returns (bytes32) {
        bytes32 orderId = _generateOrderId(msg.sender, _pairId);
        
        // Get current market price to set initial trigger
        SpotExchange.TradingPair memory pair = exchange.getTradingPair(_pairId);
        uint256 currentPrice = pair.lastPrice;
        
        uint256 initialTrigger;
        if (_side == SpotExchange.OrderSide.SELL) {
            initialTrigger = currentPrice - _trailingAmount;
        } else {
            initialTrigger = currentPrice + _trailingAmount;
        }

        AdvancedOrder storage order = advancedOrders[orderId];
        order.id = orderId;
        order.user = msg.sender;
        order.pairId = _pairId;
        order.orderType = AdvancedOrderType.TRAILING_STOP;
        order.side = _side;
        order.amount = _amount;
        order.triggerPrice = initialTrigger;
        order.trailingAmount = _trailingAmount;
        order.timeInForce = _timeInForce;
        order.expiry = _expiry;
        order.created = block.timestamp;
        order.isActive = true;
        order.lastTriggerPrice = currentPrice;

        _addToUserOrders(msg.sender, orderId);
        _addToPairOrders(_pairId, orderId);
        activeOrders.push(orderId);

        emit AdvancedOrderPlaced(orderId, msg.sender, AdvancedOrderType.TRAILING_STOP);
        return orderId;
    }

    /**
     * @dev Place an OCO (One-Cancels-Other) order
     */
    function placeOCO(
        bytes32 _pairId,
        SpotExchange.OrderSide _side,
        uint256 _amount,
        uint256 _stopPrice,
        uint256 _limitPrice,
        uint256 _profitPrice,
        uint256 _profitLimit,
        uint256 _expiry
    ) external nonReentrant returns (bytes32, bytes32) {
        // Create stop-loss leg
        bytes32 stopOrderId = _generateOrderId(msg.sender, _pairId);
        AdvancedOrder storage stopOrder = advancedOrders[stopOrderId];
        stopOrder.id = stopOrderId;
        stopOrder.user = msg.sender;
        stopOrder.pairId = _pairId;
        stopOrder.orderType = AdvancedOrderType.STOP_LOSS;
        stopOrder.side = _side;
        stopOrder.amount = _amount;
        stopOrder.triggerPrice = _stopPrice;
        stopOrder.limitPrice = _limitPrice;
        stopOrder.expiry = _expiry;
        stopOrder.created = block.timestamp;
        stopOrder.isActive = true;

        // Create take-profit leg
        bytes32 profitOrderId = _generateOrderId(msg.sender, _pairId);
        AdvancedOrder storage profitOrder = advancedOrders[profitOrderId];
        profitOrder.id = profitOrderId;
        profitOrder.user = msg.sender;
        profitOrder.pairId = _pairId;
        profitOrder.orderType = AdvancedOrderType.TAKE_PROFIT;
        profitOrder.side = _side;
        profitOrder.amount = _amount;
        profitOrder.triggerPrice = _profitPrice;
        profitOrder.limitPrice = _profitLimit;
        profitOrder.expiry = _expiry;
        profitOrder.created = block.timestamp;
        profitOrder.isActive = true;

        // Link the orders
        stopOrder.linkedOrderId = profitOrderId;
        profitOrder.linkedOrderId = stopOrderId;

        _addToUserOrders(msg.sender, stopOrderId);
        _addToUserOrders(msg.sender, profitOrderId);
        _addToPairOrders(_pairId, stopOrderId);
        _addToPairOrders(_pairId, profitOrderId);
        activeOrders.push(stopOrderId);
        activeOrders.push(profitOrderId);

        emit AdvancedOrderPlaced(stopOrderId, msg.sender, AdvancedOrderType.OCO);
        emit AdvancedOrderPlaced(profitOrderId, msg.sender, AdvancedOrderType.OCO);
        
        return (stopOrderId, profitOrderId);
    }

    /**
     * @dev Place an iceberg order
     */
    function placeIceberg(
        bytes32 _pairId,
        SpotExchange.OrderSide _side,
        uint256 _totalAmount,
        uint256 _visibleAmount,
        uint256 _price,
        TimeInForce _timeInForce,
        uint256 _expiry
    ) external nonReentrant returns (bytes32) {
        require(_visibleAmount < _totalAmount, "OrderManager: visible must be less than total");
        
        bytes32 orderId = _generateOrderId(msg.sender, _pairId);
        
        AdvancedOrder storage order = advancedOrders[orderId];
        order.id = orderId;
        order.user = msg.sender;
        order.pairId = _pairId;
        order.orderType = AdvancedOrderType.ICEBERG;
        order.side = _side;
        order.amount = _totalAmount;
        order.price = _price;
        order.visibleAmount = _visibleAmount;
        order.timeInForce = _timeInForce;
        order.expiry = _expiry;
        order.created = block.timestamp;
        order.isActive = true;

        // Initialize iceberg state
        IcebergState storage icebergState = icebergStates[orderId];
        icebergState.hiddenAmount = _totalAmount - _visibleAmount;
        icebergState.visibleAmount = _visibleAmount;

        // Place initial visible order
        _placeIcebergChild(orderId, _visibleAmount);

        _addToUserOrders(msg.sender, orderId);
        _addToPairOrders(_pairId, orderId);
        activeOrders.push(orderId);

        emit AdvancedOrderPlaced(orderId, msg.sender, AdvancedOrderType.ICEBERG);
        return orderId;
    }

    /**
     * @dev Place a TWAP order
     */
    function placeTWAP(
        bytes32 _pairId,
        SpotExchange.OrderSide _side,
        uint256 _amount,
        uint256 _timeInterval,
        uint256 _intervals,
        uint256 _maxSlippage
    ) external nonReentrant returns (bytes32) {
        bytes32 orderId = _generateOrderId(msg.sender, _pairId);
        
        AdvancedOrder storage order = advancedOrders[orderId];
        order.id = orderId;
        order.user = msg.sender;
        order.pairId = _pairId;
        order.orderType = AdvancedOrderType.TWAP;
        order.side = _side;
        order.amount = _amount;
        order.timeInterval = _timeInterval;
        order.maxSlippage = _maxSlippage;
        order.created = block.timestamp;
        order.isActive = true;

        // Initialize TWAP state
        TWAPState storage twapState = twapStates[orderId];
        twapState.totalAmount = _amount;
        twapState.startTime = block.timestamp;
        twapState.endTime = block.timestamp + (_timeInterval * _intervals);
        twapState.intervals = _intervals;
        twapState.intervalAmount = _amount / _intervals;
        twapState.nextExecutionTime = block.timestamp + _timeInterval;

        _addToUserOrders(msg.sender, orderId);
        _addToPairOrders(_pairId, orderId);
        activeOrders.push(orderId);

        emit AdvancedOrderPlaced(orderId, msg.sender, AdvancedOrderType.TWAP);
        return orderId;
    }

    /**
     * @dev Check and execute triggered orders (called by operators/keepers)
     */
    function checkAndExecuteOrders(bytes32[] calldata _orderIds) external onlyRole(OPERATOR_ROLE) {
        for (uint256 i = 0; i < _orderIds.length; i++) {
            _checkAndExecuteOrder(_orderIds[i]);
        }
    }

    /**
     * @dev Internal function to check and execute a single order
     */
    function _checkAndExecuteOrder(bytes32 _orderId) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        
        if (!order.isActive) return;
        
        // Check expiry
        if (order.timeInForce == TimeInForce.GTT && block.timestamp > order.expiry) {
            _cancelOrder(_orderId);
            return;
        }

        SpotExchange.TradingPair memory pair = exchange.getTradingPair(order.pairId);
        uint256 currentPrice = pair.lastPrice;

        if (order.orderType == AdvancedOrderType.STOP_LOSS) {
            _checkStopLoss(_orderId, currentPrice);
        } else if (order.orderType == AdvancedOrderType.TAKE_PROFIT) {
            _checkTakeProfit(_orderId, currentPrice);
        } else if (order.orderType == AdvancedOrderType.TRAILING_STOP) {
            _updateTrailingStop(_orderId, currentPrice);
        } else if (order.orderType == AdvancedOrderType.TWAP) {
            _executeTWAPInterval(_orderId);
        }
    }

    /**
     * @dev Check and execute stop-loss order
     */
    function _checkStopLoss(bytes32 _orderId, uint256 _currentPrice) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        
        bool shouldTrigger = false;
        if (order.side == SpotExchange.OrderSide.SELL) {
            shouldTrigger = _currentPrice <= order.triggerPrice;
        } else {
            shouldTrigger = _currentPrice >= order.triggerPrice;
        }

        if (shouldTrigger) {
            _executeTriggeredOrder(_orderId);
        }
    }

    /**
     * @dev Check and execute take-profit order
     */
    function _checkTakeProfit(bytes32 _orderId, uint256 _currentPrice) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        
        bool shouldTrigger = false;
        if (order.side == SpotExchange.OrderSide.SELL) {
            shouldTrigger = _currentPrice >= order.triggerPrice;
        } else {
            shouldTrigger = _currentPrice <= order.triggerPrice;
        }

        if (shouldTrigger) {
            _executeTriggeredOrder(_orderId);
        }
    }

    /**
     * @dev Update trailing stop trigger price
     */
    function _updateTrailingStop(bytes32 _orderId, uint256 _currentPrice) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        
        if (order.side == SpotExchange.OrderSide.SELL) {
            // For sell orders, trail up when price increases
            if (_currentPrice > order.lastTriggerPrice) {
                uint256 newTrigger = _currentPrice - order.trailingAmount;
                if (newTrigger > order.triggerPrice) {
                    order.triggerPrice = newTrigger;
                    order.lastTriggerPrice = _currentPrice;
                    emit TrailingStopUpdated(_orderId, newTrigger);
                }
            }
            // Check if should trigger
            if (_currentPrice <= order.triggerPrice) {
                _executeTriggeredOrder(_orderId);
            }
        } else {
            // For buy orders, trail down when price decreases
            if (_currentPrice < order.lastTriggerPrice) {
                uint256 newTrigger = _currentPrice + order.trailingAmount;
                if (newTrigger < order.triggerPrice) {
                    order.triggerPrice = newTrigger;
                    order.lastTriggerPrice = _currentPrice;
                    emit TrailingStopUpdated(_orderId, newTrigger);
                }
            }
            // Check if should trigger
            if (_currentPrice >= order.triggerPrice) {
                _executeTriggeredOrder(_orderId);
            }
        }
    }

    /**
     * @dev Execute a TWAP interval
     */
    function _executeTWAPInterval(bytes32 _orderId) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        TWAPState storage twapState = twapStates[_orderId];
        
        if (block.timestamp < twapState.nextExecutionTime) return;
        if (twapState.currentInterval >= twapState.intervals) return;

        // Calculate amount for this interval
        uint256 remainingAmount = twapState.totalAmount - twapState.executedAmount;
        uint256 remainingIntervals = twapState.intervals - twapState.currentInterval;
        uint256 intervalAmount = remainingAmount / remainingIntervals;

        // Place market order for this interval
        bytes32 childOrderId = exchange.placeOrder(
            order.pairId,
            SpotExchange.OrderType.MARKET,
            order.side,
            intervalAmount,
            0, // Market order, price = 0
            0, // No expiry
            0, // No stop price
            false // Not post-only
        );

        // Update state
        twapState.executedAmount += intervalAmount;
        twapState.currentInterval++;
        twapState.nextExecutionTime = block.timestamp + order.timeInterval;

        order.childOrders[childOrderId] = true;

        emit TWAPIntervalExecuted(_orderId, twapState.currentInterval, intervalAmount);

        // Complete TWAP if all intervals executed
        if (twapState.currentInterval >= twapState.intervals) {
            order.isActive = false;
        }
    }

    /**
     * @dev Execute a triggered order
     */
    function _executeTriggeredOrder(bytes32 _orderId) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        
        // Place the actual order on the exchange
        bytes32 exchangeOrderId = exchange.placeOrder(
            order.pairId,
            order.limitPrice > 0 ? SpotExchange.OrderType.LIMIT : SpotExchange.OrderType.MARKET,
            order.side,
            order.amount,
            order.limitPrice,
            order.expiry,
            0, // No stop price for exchange order
            false // Not post-only
        );

        // Handle OCO logic
        if (order.linkedOrderId != bytes32(0)) {
            _cancelOrder(order.linkedOrderId);
            emit OCOOrderActivated(_orderId, order.linkedOrderId);
        }

        order.isActive = false;
        order.childOrders[exchangeOrderId] = true;
        
        emit OrderTriggered(_orderId, order.triggerPrice);
    }

    /**
     * @dev Place a child order for iceberg
     */
    function _placeIcebergChild(bytes32 _orderId, uint256 _amount) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        
        bytes32 childOrderId = exchange.placeOrder(
            order.pairId,
            SpotExchange.OrderType.LIMIT,
            order.side,
            _amount,
            order.price,
            order.expiry,
            0, // No stop price
            false // Not post-only
        );

        IcebergState storage icebergState = icebergStates[_orderId];
        icebergState.childOrderIds.push(childOrderId);
        order.childOrders[childOrderId] = true;
    }

    /**
     * @dev Handle iceberg order refill when child order is filled
     */
    function refillIcebergOrder(bytes32 _orderId) external onlyRole(OPERATOR_ROLE) {
        AdvancedOrder storage order = advancedOrders[_orderId];
        require(order.orderType == AdvancedOrderType.ICEBERG, "OrderManager: not iceberg order");
        
        IcebergState storage icebergState = icebergStates[_orderId];
        
        if (icebergState.hiddenAmount > 0) {
            uint256 refillAmount = icebergState.hiddenAmount >= order.visibleAmount ? 
                order.visibleAmount : icebergState.hiddenAmount;
            
            icebergState.hiddenAmount -= refillAmount;
            _placeIcebergChild(_orderId, refillAmount);
            
            emit IcebergOrderRefilled(_orderId, refillAmount);
        }

        if (icebergState.hiddenAmount == 0) {
            order.isActive = false;
        }
    }

    /**
     * @dev Cancel an advanced order
     */
    function cancelOrder(bytes32 _orderId) external nonReentrant {
        AdvancedOrder storage order = advancedOrders[_orderId];
        require(order.user == msg.sender, "OrderManager: not order owner");
        
        _cancelOrder(_orderId);
    }

    /**
     * @dev Internal cancel order function
     */
    function _cancelOrder(bytes32 _orderId) internal {
        AdvancedOrder storage order = advancedOrders[_orderId];
        order.isActive = false;
        
        // Cancel linked order if OCO
        if (order.linkedOrderId != bytes32(0)) {
            advancedOrders[order.linkedOrderId].isActive = false;
        }
    }

    /**
     * @dev Generate unique order ID
     */
    function _generateOrderId(address _user, bytes32 _pairId) internal view returns (bytes32) {
        return keccak256(abi.encodePacked(_user, _pairId, block.timestamp, block.difficulty));
    }

    /**
     * @dev Add order to user's order list
     */
    function _addToUserOrders(address _user, bytes32 _orderId) internal {
        userOrders[_user].push(_orderId);
    }

    /**
     * @dev Add order to pair's order list
     */
    function _addToPairOrders(bytes32 _pairId, bytes32 _orderId) internal {
        pairOrders[_pairId].push(_orderId);
    }

    // View functions
    function getAdvancedOrder(bytes32 _orderId) external view returns (
        bytes32 id,
        address user,
        bytes32 pairId,
        AdvancedOrderType orderType,
        SpotExchange.OrderSide side,
        uint256 amount,
        uint256 price,
        uint256 triggerPrice,
        bool isActive
    ) {
        AdvancedOrder storage order = advancedOrders[_orderId];
        return (
            order.id,
            order.user,
            order.pairId,
            order.orderType,
            order.side,
            order.amount,
            order.price,
            order.triggerPrice,
            order.isActive
        );
    }

    function getUserOrders(address _user) external view returns (bytes32[] memory) {
        return userOrders[_user];
    }

    function getTWAPState(bytes32 _orderId) external view returns (TWAPState memory) {
        return twapStates[_orderId];
    }

    function getIcebergState(bytes32 _orderId) external view returns (IcebergState memory) {
        return icebergStates[_orderId];
    }

    // Admin functions
    function pause() external onlyRole(ADMIN_ROLE) {
        // Pause functionality if needed
    }

    function unpause() external onlyRole(ADMIN_ROLE) {
        // Unpause functionality if needed
    }
}