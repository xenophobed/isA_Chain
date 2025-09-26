// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";

/**
 * @title PriceOracle
 * @dev Decentralized price oracle with multiple data sources and aggregation
 * 
 * Features:
 * - Multi-source price aggregation
 * - Oracle node management and reputation system
 * - Time-weighted average prices (TWAP)
 * - Price deviation detection and circuit breakers
 * - Heartbeat mechanism for data freshness
 * - Slashing for malicious or inaccurate data
 */
contract PriceOracle is AccessControl, Pausable, ReentrancyGuard {
    // Roles
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant ORACLE_ROLE = keccak256("ORACLE_ROLE");
    bytes32 public constant VALIDATOR_ROLE = keccak256("VALIDATOR_ROLE");

    // Constants
    uint256 public constant PRECISION = 1e18;
    uint256 public constant MAX_DEVIATION = 500; // 5% maximum price deviation
    uint256 public constant MIN_ORACLE_COUNT = 3;
    uint256 public constant MAX_ORACLE_COUNT = 21;
    uint256 public constant HEARTBEAT_INTERVAL = 1 hours;
    uint256 public constant STALENESS_THRESHOLD = 3600; // 1 hour
    uint256 public constant SLASH_AMOUNT = 100 * 1e18; // 100 tokens

    // Oracle node information
    struct OracleNode {
        address nodeAddress;
        bool isActive;
        uint256 reputation;
        uint256 totalSubmissions;
        uint256 accurateSubmissions;
        uint256 stake;
        uint256 lastSubmissionTime;
        uint256 slashCount;
        string endpoint;
        bytes32[] supportedFeeds;
    }

    // Price feed information
    struct PriceFeed {
        string symbol;
        address asset;
        uint256 latestPrice;
        uint256 timestamp;
        uint256 roundId;
        bool isActive;
        uint256 heartbeat;
        uint256 deviationThreshold;
        uint256 minOracleCount;
        mapping(uint256 => PriceRound) rounds;
        uint256 currentRoundId;
    }

    // Individual price submission
    struct PriceSubmission {
        address oracle;
        uint256 price;
        uint256 timestamp;
        bool isValid;
    }

    // Price round aggregation
    struct PriceRound {
        uint256 roundId;
        uint256 price;
        uint256 timestamp;
        uint256 submissionCount;
        uint256 validSubmissionCount;
        bool isFinalized;
        mapping(address => PriceSubmission) submissions;
        address[] submitters;
    }

    // TWAP calculation data
    struct TWAPData {
        uint256 price;
        uint256 timestamp;
        uint256 accumulator;
        uint256 windowSize;
    }

    // State variables
    mapping(bytes32 => PriceFeed) public priceFeeds;
    mapping(address => OracleNode) public oracleNodes;
    mapping(bytes32 => TWAPData) public twapData;
    mapping(address => uint256) public stakes;
    
    bytes32[] public feedIds;
    address[] public activeOracles;
    address public stakingToken;
    uint256 public minStakeAmount;
    address public treasury;
    
    // Events
    event FeedAdded(bytes32 indexed feedId, string symbol, address asset);
    event PriceSubmitted(bytes32 indexed feedId, address indexed oracle, uint256 price, uint256 roundId);
    event PriceUpdated(bytes32 indexed feedId, uint256 price, uint256 roundId, uint256 timestamp);
    event OracleAdded(address indexed oracle, uint256 stake);
    event OracleSlashed(address indexed oracle, uint256 amount, string reason);
    event HeartbeatMissed(bytes32 indexed feedId, address indexed oracle);
    event TWAPUpdated(bytes32 indexed feedId, uint256 twapPrice, uint256 windowSize);

    /**
     * @dev Constructor
     */
    constructor(address _stakingToken, address _treasury, uint256 _minStakeAmount) {
        require(_stakingToken != address(0), "PriceOracle: invalid staking token");
        require(_treasury != address(0), "PriceOracle: invalid treasury");
        
        stakingToken = _stakingToken;
        treasury = _treasury;
        minStakeAmount = _minStakeAmount;
        
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ADMIN_ROLE, msg.sender);
        _grantRole(VALIDATOR_ROLE, msg.sender);
    }

    /**
     * @dev Add a new price feed
     */
    function addPriceFeed(
        bytes32 _feedId,
        string calldata _symbol,
        address _asset,
        uint256 _heartbeat,
        uint256 _deviationThreshold,
        uint256 _minOracleCount
    ) external onlyRole(ADMIN_ROLE) {
        require(priceFeeds[_feedId].asset == address(0), "PriceOracle: feed already exists");
        require(_asset != address(0), "PriceOracle: invalid asset");
        require(_minOracleCount >= MIN_ORACLE_COUNT, "PriceOracle: insufficient oracle count");
        require(_deviationThreshold <= 1000, "PriceOracle: deviation threshold too high");

        PriceFeed storage feed = priceFeeds[_feedId];
        feed.symbol = _symbol;
        feed.asset = _asset;
        feed.isActive = true;
        feed.heartbeat = _heartbeat;
        feed.deviationThreshold = _deviationThreshold;
        feed.minOracleCount = _minOracleCount;
        feed.currentRoundId = 1;

        feedIds.push(_feedId);
        emit FeedAdded(_feedId, _symbol, _asset);
    }

    /**
     * @dev Register as an oracle node
     */
    function registerOracle(
        uint256 _stakeAmount,
        string calldata _endpoint,
        bytes32[] calldata _supportedFeeds
    ) external nonReentrant {
        require(_stakeAmount >= minStakeAmount, "PriceOracle: insufficient stake");
        require(activeOracles.length < MAX_ORACLE_COUNT, "PriceOracle: max oracles reached");
        require(oracleNodes[msg.sender].nodeAddress == address(0), "PriceOracle: already registered");

        // Transfer stake
        IERC20(stakingToken).transferFrom(msg.sender, address(this), _stakeAmount);
        stakes[msg.sender] = _stakeAmount;

        // Register oracle
        OracleNode storage oracle = oracleNodes[msg.sender];
        oracle.nodeAddress = msg.sender;
        oracle.isActive = true;
        oracle.reputation = 1000; // Starting reputation
        oracle.stake = _stakeAmount;
        oracle.endpoint = _endpoint;
        oracle.supportedFeeds = _supportedFeeds;

        activeOracles.push(msg.sender);
        _grantRole(ORACLE_ROLE, msg.sender);

        emit OracleAdded(msg.sender, _stakeAmount);
    }

    /**
     * @dev Submit price data
     */
    function submitPrice(
        bytes32 _feedId,
        uint256 _price,
        uint256 _timestamp
    ) external onlyRole(ORACLE_ROLE) whenNotPaused {
        require(priceFeeds[_feedId].isActive, "PriceOracle: feed not active");
        require(_price > 0, "PriceOracle: invalid price");
        require(_timestamp <= block.timestamp, "PriceOracle: future timestamp");
        require(block.timestamp - _timestamp <= STALENESS_THRESHOLD, "PriceOracle: stale data");

        PriceFeed storage feed = priceFeeds[_feedId];
        OracleNode storage oracle = oracleNodes[msg.sender];
        
        // Check if oracle supports this feed
        require(_supportsfeed(msg.sender, _feedId), "PriceOracle: feed not supported");

        // Get or create current round
        uint256 currentRound = feed.currentRoundId;
        PriceRound storage round = feed.rounds[currentRound];
        
        // Initialize round if first submission
        if (round.roundId == 0) {
            round.roundId = currentRound;
            round.timestamp = block.timestamp;
        }

        // Check if oracle already submitted for this round
        require(round.submissions[msg.sender].timestamp == 0, "PriceOracle: already submitted");

        // Validate price against existing submissions
        bool isValid = _validatePrice(_feedId, _price, currentRound);

        // Store submission
        round.submissions[msg.sender] = PriceSubmission({
            oracle: msg.sender,
            price: _price,
            timestamp: _timestamp,
            isValid: isValid
        });

        round.submitters.push(msg.sender);
        round.submissionCount++;
        
        if (isValid) {
            round.validSubmissionCount++;
        }

        // Update oracle stats
        oracle.totalSubmissions++;
        oracle.lastSubmissionTime = block.timestamp;
        
        if (isValid) {
            oracle.accurateSubmissions++;
            oracle.reputation = _updateReputation(oracle.reputation, true);
        } else {
            oracle.reputation = _updateReputation(oracle.reputation, false);
        }

        emit PriceSubmitted(_feedId, msg.sender, _price, currentRound);

        // Try to finalize round if enough valid submissions
        if (round.validSubmissionCount >= feed.minOracleCount) {
            _finalizeRound(_feedId, currentRound);
        }
    }

    /**
     * @dev Finalize a price round
     */
    function _finalizeRound(bytes32 _feedId, uint256 _roundId) internal {
        PriceFeed storage feed = priceFeeds[_feedId];
        PriceRound storage round = feed.rounds[_roundId];

        require(!round.isFinalized, "PriceOracle: round already finalized");
        require(round.validSubmissionCount >= feed.minOracleCount, "PriceOracle: insufficient valid submissions");

        // Aggregate prices from valid submissions
        uint256 aggregatedPrice = _aggregatePrices(_feedId, _roundId);
        
        // Update feed data
        feed.latestPrice = aggregatedPrice;
        feed.timestamp = block.timestamp;
        feed.roundId = _roundId;
        
        // Finalize round
        round.price = aggregatedPrice;
        round.isFinalized = true;
        
        // Start next round
        feed.currentRoundId++;

        // Update TWAP
        _updateTWAP(_feedId, aggregatedPrice);

        emit PriceUpdated(_feedId, aggregatedPrice, _roundId, block.timestamp);
    }

    /**
     * @dev Aggregate prices from valid submissions
     */
    function _aggregatePrices(bytes32 _feedId, uint256 _roundId) internal view returns (uint256) {
        PriceRound storage round = priceFeeds[_feedId].rounds[_roundId];
        
        uint256[] memory validPrices = new uint256[](round.validSubmissionCount);
        uint256 validCount = 0;

        // Collect valid prices
        for (uint256 i = 0; i < round.submitters.length; i++) {
            address submitter = round.submitters[i];
            PriceSubmission storage submission = round.submissions[submitter];
            
            if (submission.isValid) {
                validPrices[validCount] = submission.price;
                validCount++;
            }
        }

        // Sort prices and take median
        _quickSort(validPrices, 0, int256(validCount - 1));
        
        if (validCount % 2 == 1) {
            return validPrices[validCount / 2];
        } else {
            return (validPrices[validCount / 2 - 1] + validPrices[validCount / 2]) / 2;
        }
    }

    /**
     * @dev Quick sort implementation for price sorting
     */
    function _quickSort(uint256[] memory arr, int256 left, int256 right) internal pure {
        if (left < right) {
            int256 pivotIndex = _partition(arr, left, right);
            _quickSort(arr, left, pivotIndex - 1);
            _quickSort(arr, pivotIndex + 1, right);
        }
    }

    function _partition(uint256[] memory arr, int256 left, int256 right) internal pure returns (int256) {
        uint256 pivot = arr[uint256(right)];
        int256 i = left - 1;

        for (int256 j = left; j < right; j++) {
            if (arr[uint256(j)] <= pivot) {
                i++;
                (arr[uint256(i)], arr[uint256(j)]) = (arr[uint256(j)], arr[uint256(i)]);
            }
        }
        (arr[uint256(i + 1)], arr[uint256(right)]) = (arr[uint256(right)], arr[uint256(i + 1)]);
        return i + 1;
    }

    /**
     * @dev Validate price against existing submissions
     */
    function _validatePrice(bytes32 _feedId, uint256 _price, uint256 _roundId) internal view returns (bool) {
        PriceRound storage round = priceFeeds[_feedId].rounds[_roundId];
        
        if (round.validSubmissionCount == 0) {
            return true; // First submission is always valid
        }

        // Check deviation against median of existing valid prices
        uint256 medianPrice = _getMedianPrice(_feedId, _roundId);
        uint256 deviation = _price > medianPrice ? 
            ((_price - medianPrice) * 10000) / medianPrice :
            ((medianPrice - _price) * 10000) / medianPrice;

        return deviation <= priceFeeds[_feedId].deviationThreshold;
    }

    /**
     * @dev Get median price from current round submissions
     */
    function _getMedianPrice(bytes32 _feedId, uint256 _roundId) internal view returns (uint256) {
        PriceRound storage round = priceFeeds[_feedId].rounds[_roundId];
        
        if (round.validSubmissionCount == 0) {
            return 0;
        }

        uint256[] memory prices = new uint256[](round.validSubmissionCount);
        uint256 count = 0;

        for (uint256 i = 0; i < round.submitters.length; i++) {
            PriceSubmission storage submission = round.submissions[round.submitters[i]];
            if (submission.isValid) {
                prices[count] = submission.price;
                count++;
            }
        }

        // Simple bubble sort for small arrays
        for (uint256 i = 0; i < count - 1; i++) {
            for (uint256 j = 0; j < count - i - 1; j++) {
                if (prices[j] > prices[j + 1]) {
                    (prices[j], prices[j + 1]) = (prices[j + 1], prices[j]);
                }
            }
        }

        return count % 2 == 1 ? prices[count / 2] : (prices[count / 2 - 1] + prices[count / 2]) / 2;
    }

    /**
     * @dev Update TWAP data
     */
    function _updateTWAP(bytes32 _feedId, uint256 _price) internal {
        TWAPData storage twap = twapData[_feedId];
        uint256 timeElapsed = block.timestamp - twap.timestamp;

        if (twap.timestamp == 0) {
            // First price update
            twap.price = _price;
            twap.timestamp = block.timestamp;
            twap.accumulator = _price * block.timestamp;
            twap.windowSize = 3600; // 1 hour default
        } else {
            // Update accumulator and calculate TWAP
            twap.accumulator += twap.price * timeElapsed;
            
            if (timeElapsed >= twap.windowSize) {
                twap.price = twap.accumulator / (twap.timestamp + timeElapsed - (twap.timestamp + timeElapsed - twap.windowSize));
                twap.accumulator = _price * block.timestamp;
                twap.timestamp = block.timestamp;
            }
        }

        emit TWAPUpdated(_feedId, twap.price, twap.windowSize);
    }

    /**
     * @dev Update oracle reputation
     */
    function _updateReputation(uint256 _currentRep, bool _accurate) internal pure returns (uint256) {
        if (_accurate) {
            return _currentRep < 2000 ? _currentRep + 10 : 2000; // Cap at 2000
        } else {
            return _currentRep > 10 ? _currentRep - 20 : 0; // Floor at 0
        }
    }

    /**
     * @dev Check if oracle supports a feed
     */
    function _supportsFeeds(address _oracle, bytes32 _feedId) internal view returns (bool) {
        bytes32[] memory supportedFeeds = oracleNodes[_oracle].supportedFeeds;
        for (uint256 i = 0; i < supportedFeeds.length; i++) {
            if (supportedFeeds[i] == _feedId) {
                return true;
            }
        }
        return false;
    }

    // View functions
    function getLatestPrice(bytes32 _feedId) external view returns (uint256 price, uint256 timestamp) {
        PriceFeed storage feed = priceFeeds[_feedId];
        return (feed.latestPrice, feed.timestamp);
    }

    function getTWAP(bytes32 _feedId) external view returns (uint256) {
        return twapData[_feedId].price;
    }

    function getRoundData(bytes32 _feedId, uint256 _roundId) external view returns (
        uint256 price,
        uint256 timestamp,
        uint256 submissionCount,
        bool isFinalized
    ) {
        PriceRound storage round = priceFeeds[_feedId].rounds[_roundId];
        return (round.price, round.timestamp, round.submissionCount, round.isFinalized);
    }

    function getOracleInfo(address _oracle) external view returns (OracleNode memory) {
        return oracleNodes[_oracle];
    }

    function getFeedInfo(bytes32 _feedId) external view returns (
        string memory symbol,
        address asset,
        uint256 latestPrice,
        uint256 timestamp,
        bool isActive
    ) {
        PriceFeed storage feed = priceFeeds[_feedId];
        return (feed.symbol, feed.asset, feed.latestPrice, feed.timestamp, feed.isActive);
    }

    // Admin functions
    function slashOracle(address _oracle, string calldata _reason) external onlyRole(VALIDATOR_ROLE) {
        OracleNode storage oracle = oracleNodes[_oracle];
        require(oracle.isActive, "PriceOracle: oracle not active");
        require(oracle.stake >= SLASH_AMOUNT, "PriceOracle: insufficient stake");

        oracle.stake -= SLASH_AMOUNT;
        oracle.slashCount++;
        oracle.reputation = oracle.reputation > 100 ? oracle.reputation - 100 : 0;

        // Transfer slashed amount to treasury
        stakes[_oracle] -= SLASH_AMOUNT;
        IERC20(stakingToken).transfer(treasury, SLASH_AMOUNT);

        // Deactivate oracle if slashed too many times or stake too low
        if (oracle.slashCount >= 3 || oracle.stake < minStakeAmount) {
            oracle.isActive = false;
            _revokeRole(ORACLE_ROLE, _oracle);
        }

        emit OracleSlashed(_oracle, SLASH_AMOUNT, _reason);
    }

    function setFeedActive(bytes32 _feedId, bool _isActive) external onlyRole(ADMIN_ROLE) {
        priceFeeds[_feedId].isActive = _isActive;
    }

    function setMinStakeAmount(uint256 _amount) external onlyRole(ADMIN_ROLE) {
        minStakeAmount = _amount;
    }

    function pause() external onlyRole(ADMIN_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(ADMIN_ROLE) {
        _unpause();
    }
}