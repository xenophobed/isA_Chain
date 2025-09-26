// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/utils/math/Math.sol";

/**
 * @title YieldFarming
 * @dev Advanced yield farming protocol for liquidity mining
 * 
 * Features:
 * - Multiple farming pools with different reward tokens
 * - Flexible reward distribution schedules
 * - Boost multipliers based on lock duration
 * - Emergency withdrawal with penalties
 * - Compound reward claiming
 * - Pool weight adjustments
 */
contract YieldFarming is AccessControl, ReentrancyGuard, Pausable {
    using SafeERC20 for IERC20;
    using Math for uint256;

    // Roles
    bytes32 public constant ADMIN_ROLE = keccak256("ADMIN_ROLE");
    bytes32 public constant FARM_MANAGER_ROLE = keccak256("FARM_MANAGER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    // Constants
    uint256 public constant PRECISION = 1e18;
    uint256 public constant MAX_BOOST_MULTIPLIER = 25000; // 2.5x max boost
    uint256 public constant MIN_LOCK_DURATION = 1 days;
    uint256 public constant MAX_LOCK_DURATION = 365 days;
    uint256 public constant EMERGENCY_PENALTY = 1000; // 10% penalty for emergency withdrawal

    // Pool information
    struct PoolInfo {
        IERC20 stakingToken;
        IERC20 rewardToken;
        uint256 totalStaked;
        uint256 rewardPerSecond;
        uint256 startTime;
        uint256 endTime;
        uint256 lastRewardTime;
        uint256 accRewardPerShare;
        uint256 allocPoint;
        bool isActive;
        uint256 minStakeAmount;
        uint256 maxStakeAmount;
        uint256 emergencyWithdrawPenalty;
        bool allowCompounding;
        uint256 lockDuration; // Required lock duration
    }

    // User information
    struct UserInfo {
        uint256 amount;
        uint256 rewardDebt;
        uint256 pendingRewards;
        uint256 stakeTime;
        uint256 lockEndTime;
        uint256 boostMultiplier;
        uint256 totalRewardsClaimed;
        bool autoCompound;
    }

    // Boost tier information
    struct BoostTier {
        uint256 lockDuration;
        uint256 multiplier; // In basis points (10000 = 1x)
        bool isActive;
    }

    // State variables
    PoolInfo[] public poolInfo;
    mapping(uint256 => mapping(address => UserInfo)) public userInfo;
    mapping(uint256 => BoostTier[]) public boostTiers;
    mapping(address => bool) public authorizedCompounders;
    
    uint256 public totalAllocPoint;
    address public treasury;
    uint256 public emergencyWithdrawFeeCollected;
    
    // Events
    event PoolAdded(uint256 indexed pid, address stakingToken, address rewardToken, uint256 allocPoint);
    event Stake(address indexed user, uint256 indexed pid, uint256 amount);
    event Withdraw(address indexed user, uint256 indexed pid, uint256 amount);
    event EmergencyWithdraw(address indexed user, uint256 indexed pid, uint256 amount, uint256 penalty);
    event Harvest(address indexed user, uint256 indexed pid, uint256 amount);
    event Compound(address indexed user, uint256 indexed pid, uint256 amount);
    event PoolUpdated(uint256 indexed pid, uint256 rewardPerSecond, uint256 allocPoint);
    event BoostTierAdded(uint256 indexed pid, uint256 lockDuration, uint256 multiplier);
    event AutoCompoundToggled(address indexed user, uint256 indexed pid, bool enabled);

    /**
     * @dev Constructor
     * @param _treasury Treasury address for fees
     */
    constructor(address _treasury) {
        require(_treasury != address(0), "YieldFarming: invalid treasury");
        
        treasury = _treasury;
        
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(ADMIN_ROLE, msg.sender);
        _grantRole(FARM_MANAGER_ROLE, msg.sender);
        _grantRole(PAUSER_ROLE, msg.sender);
    }

    /**
     * @dev Add a new farming pool
     */
    function addPool(
        IERC20 _stakingToken,
        IERC20 _rewardToken,
        uint256 _rewardPerSecond,
        uint256 _allocPoint,
        uint256 _startTime,
        uint256 _endTime,
        uint256 _minStakeAmount,
        uint256 _maxStakeAmount,
        uint256 _lockDuration,
        bool _allowCompounding
    ) external onlyRole(FARM_MANAGER_ROLE) {
        require(address(_stakingToken) != address(0), "YieldFarming: invalid staking token");
        require(address(_rewardToken) != address(0), "YieldFarming: invalid reward token");
        require(_startTime >= block.timestamp, "YieldFarming: start time must be in future");
        require(_endTime > _startTime, "YieldFarming: end time must be after start time");
        require(_lockDuration >= MIN_LOCK_DURATION && _lockDuration <= MAX_LOCK_DURATION, "YieldFarming: invalid lock duration");

        _updateAllPools();

        totalAllocPoint += _allocPoint;

        poolInfo.push(PoolInfo({
            stakingToken: _stakingToken,
            rewardToken: _rewardToken,
            totalStaked: 0,
            rewardPerSecond: _rewardPerSecond,
            startTime: _startTime,
            endTime: _endTime,
            lastRewardTime: _startTime,
            accRewardPerShare: 0,
            allocPoint: _allocPoint,
            isActive: true,
            minStakeAmount: _minStakeAmount,
            maxStakeAmount: _maxStakeAmount,
            emergencyWithdrawPenalty: EMERGENCY_PENALTY,
            allowCompounding: _allowCompounding,
            lockDuration: _lockDuration
        }));

        uint256 pid = poolInfo.length - 1;

        // Add default boost tiers
        _addBoostTier(pid, 0, 10000); // No lock = 1x
        _addBoostTier(pid, 30 days, 12000); // 30 days = 1.2x
        _addBoostTier(pid, 90 days, 15000); // 90 days = 1.5x
        _addBoostTier(pid, 180 days, 20000); // 180 days = 2x
        _addBoostTier(pid, 365 days, 25000); // 365 days = 2.5x

        emit PoolAdded(pid, address(_stakingToken), address(_rewardToken), _allocPoint);
    }

    /**
     * @dev Stake tokens in a farming pool
     */
    function stake(uint256 _pid, uint256 _amount, uint256 _lockDuration) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        require(_amount > 0, "YieldFarming: amount must be positive");

        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][msg.sender];

        require(pool.isActive, "YieldFarming: pool not active");
        require(block.timestamp >= pool.startTime, "YieldFarming: pool not started");
        require(block.timestamp < pool.endTime, "YieldFarming: pool ended");
        require(_amount >= pool.minStakeAmount, "YieldFarming: below minimum stake");
        require(pool.maxStakeAmount == 0 || user.amount + _amount <= pool.maxStakeAmount, "YieldFarming: exceeds maximum stake");
        require(_lockDuration >= pool.lockDuration, "YieldFarming: lock duration too short");

        _updatePool(_pid);

        // Calculate pending rewards and transfer to user
        if (user.amount > 0) {
            uint256 pending = (user.amount * pool.accRewardPerShare / PRECISION) - user.rewardDebt;
            if (pending > 0) {
                user.pendingRewards += pending;
            }
        }

        // Calculate boost multiplier based on lock duration
        uint256 boostMultiplier = _getBoostMultiplier(_pid, _lockDuration);

        // Update user info
        user.amount += _amount;
        user.stakeTime = block.timestamp;
        user.lockEndTime = block.timestamp + _lockDuration;
        user.boostMultiplier = boostMultiplier;
        user.rewardDebt = user.amount * pool.accRewardPerShare / PRECISION;

        // Update pool info
        pool.totalStaked += _amount;

        // Transfer staking tokens from user
        pool.stakingToken.safeTransferFrom(msg.sender, address(this), _amount);

        emit Stake(msg.sender, _pid, _amount);
    }

    /**
     * @dev Withdraw staked tokens
     */
    function withdraw(uint256 _pid, uint256 _amount) 
        external 
        nonReentrant 
        whenNotPaused 
    {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        
        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][msg.sender];
        
        require(user.amount >= _amount, "YieldFarming: insufficient balance");
        require(block.timestamp >= user.lockEndTime, "YieldFarming: still locked");

        _updatePool(_pid);

        // Calculate and transfer pending rewards
        uint256 pending = (user.amount * pool.accRewardPerShare / PRECISION) - user.rewardDebt;
        if (pending > 0 || user.pendingRewards > 0) {
            uint256 totalRewards = pending + user.pendingRewards;
            _transferRewards(_pid, msg.sender, totalRewards);
            user.pendingRewards = 0;
            user.totalRewardsClaimed += totalRewards;
        }

        // Update user and pool state
        user.amount -= _amount;
        user.rewardDebt = user.amount * pool.accRewardPerShare / PRECISION;
        pool.totalStaked -= _amount;

        // Transfer staked tokens back to user
        pool.stakingToken.safeTransfer(msg.sender, _amount);

        emit Withdraw(msg.sender, _pid, _amount);
    }

    /**
     * @dev Emergency withdraw with penalty
     */
    function emergencyWithdraw(uint256 _pid) external nonReentrant {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        
        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][msg.sender];
        
        require(user.amount > 0, "YieldFarming: no stake to withdraw");

        uint256 amount = user.amount;
        uint256 penalty = (amount * pool.emergencyWithdrawPenalty) / 10000;
        uint256 withdrawAmount = amount - penalty;

        // Reset user state
        user.amount = 0;
        user.rewardDebt = 0;
        user.pendingRewards = 0;
        user.lockEndTime = 0;

        // Update pool state
        pool.totalStaked -= amount;

        // Collect penalty
        emergencyWithdrawFeeCollected += penalty;

        // Transfer tokens
        if (penalty > 0) {
            pool.stakingToken.safeTransfer(treasury, penalty);
        }
        pool.stakingToken.safeTransfer(msg.sender, withdrawAmount);

        emit EmergencyWithdraw(msg.sender, _pid, withdrawAmount, penalty);
    }

    /**
     * @dev Harvest rewards without withdrawing stake
     */
    function harvest(uint256 _pid) external nonReentrant whenNotPaused {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        
        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][msg.sender];

        _updatePool(_pid);

        uint256 pending = (user.amount * pool.accRewardPerShare / PRECISION) - user.rewardDebt;
        uint256 totalRewards = pending + user.pendingRewards;

        if (totalRewards > 0) {
            // Handle auto-compound if enabled
            if (user.autoCompound && pool.allowCompounding && address(pool.stakingToken) == address(pool.rewardToken)) {
                _compound(_pid, msg.sender, totalRewards);
            } else {
                _transferRewards(_pid, msg.sender, totalRewards);
                user.totalRewardsClaimed += totalRewards;
                emit Harvest(msg.sender, _pid, totalRewards);
            }

            user.pendingRewards = 0;
        }

        user.rewardDebt = user.amount * pool.accRewardPerShare / PRECISION;
    }

    /**
     * @dev Compound rewards back into the pool
     */
    function compound(uint256 _pid) external nonReentrant whenNotPaused {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        
        PoolInfo storage pool = poolInfo[_pid];
        require(pool.allowCompounding, "YieldFarming: compounding not allowed");
        require(address(pool.stakingToken) == address(pool.rewardToken), "YieldFarming: different tokens");

        UserInfo storage user = userInfo[_pid][msg.sender];

        _updatePool(_pid);

        uint256 pending = (user.amount * pool.accRewardPerShare / PRECISION) - user.rewardDebt;
        uint256 totalRewards = pending + user.pendingRewards;

        require(totalRewards > 0, "YieldFarming: no rewards to compound");

        _compound(_pid, msg.sender, totalRewards);
    }

    /**
     * @dev Internal compound function
     */
    function _compound(uint256 _pid, address _user, uint256 _amount) internal {
        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][_user];

        // Add rewards to user's stake
        user.amount += _amount;
        pool.totalStaked += _amount;
        
        // Reset reward debt and pending rewards
        user.rewardDebt = user.amount * pool.accRewardPerShare / PRECISION;
        user.pendingRewards = 0;

        emit Compound(_user, _pid, _amount);
    }

    /**
     * @dev Toggle auto-compound for a user
     */
    function toggleAutoCompound(uint256 _pid) external {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        require(poolInfo[_pid].allowCompounding, "YieldFarming: compounding not allowed");
        
        UserInfo storage user = userInfo[_pid][msg.sender];
        user.autoCompound = !user.autoCompound;
        
        emit AutoCompoundToggled(msg.sender, _pid, user.autoCompound);
    }

    /**
     * @dev Update pool rewards
     */
    function _updatePool(uint256 _pid) internal {
        PoolInfo storage pool = poolInfo[_pid];
        
        if (block.timestamp <= pool.lastRewardTime) {
            return;
        }

        if (pool.totalStaked == 0) {
            pool.lastRewardTime = block.timestamp;
            return;
        }

        uint256 endTime = block.timestamp < pool.endTime ? block.timestamp : pool.endTime;
        uint256 multiplier = endTime - pool.lastRewardTime;
        uint256 reward = multiplier * pool.rewardPerSecond * pool.allocPoint / totalAllocPoint;

        pool.accRewardPerShare += (reward * PRECISION) / pool.totalStaked;
        pool.lastRewardTime = endTime;
    }

    /**
     * @dev Update all pools
     */
    function _updateAllPools() internal {
        for (uint256 pid = 0; pid < poolInfo.length; pid++) {
            _updatePool(pid);
        }
    }

    /**
     * @dev Transfer rewards with boost multiplier
     */
    function _transferRewards(uint256 _pid, address _to, uint256 _amount) internal {
        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][_to];
        
        // Apply boost multiplier
        uint256 boostedAmount = (_amount * user.boostMultiplier) / 10000;
        
        pool.rewardToken.safeTransfer(_to, boostedAmount);
    }

    /**
     * @dev Add boost tier to a pool
     */
    function _addBoostTier(uint256 _pid, uint256 _lockDuration, uint256 _multiplier) internal {
        boostTiers[_pid].push(BoostTier({
            lockDuration: _lockDuration,
            multiplier: _multiplier,
            isActive: true
        }));
    }

    /**
     * @dev Get boost multiplier for lock duration
     */
    function _getBoostMultiplier(uint256 _pid, uint256 _lockDuration) internal view returns (uint256) {
        BoostTier[] storage tiers = boostTiers[_pid];
        uint256 bestMultiplier = 10000; // Default 1x

        for (uint256 i = 0; i < tiers.length; i++) {
            if (tiers[i].isActive && _lockDuration >= tiers[i].lockDuration && tiers[i].multiplier > bestMultiplier) {
                bestMultiplier = tiers[i].multiplier;
            }
        }

        return bestMultiplier;
    }

    // View functions
    function poolLength() external view returns (uint256) {
        return poolInfo.length;
    }

    function pendingRewards(uint256 _pid, address _user) external view returns (uint256) {
        PoolInfo storage pool = poolInfo[_pid];
        UserInfo storage user = userInfo[_pid][_user];
        
        uint256 accRewardPerShare = pool.accRewardPerShare;
        
        if (block.timestamp > pool.lastRewardTime && pool.totalStaked > 0) {
            uint256 endTime = block.timestamp < pool.endTime ? block.timestamp : pool.endTime;
            uint256 multiplier = endTime - pool.lastRewardTime;
            uint256 reward = multiplier * pool.rewardPerSecond * pool.allocPoint / totalAllocPoint;
            accRewardPerShare += (reward * PRECISION) / pool.totalStaked;
        }
        
        uint256 pending = (user.amount * accRewardPerShare / PRECISION) - user.rewardDebt;
        uint256 totalPending = pending + user.pendingRewards;
        
        // Apply boost multiplier
        return (totalPending * user.boostMultiplier) / 10000;
    }

    function getPoolInfo(uint256 _pid) external view returns (PoolInfo memory) {
        return poolInfo[_pid];
    }

    function getUserInfo(uint256 _pid, address _user) external view returns (UserInfo memory) {
        return userInfo[_pid][_user];
    }

    function getBoostTiers(uint256 _pid) external view returns (BoostTier[] memory) {
        return boostTiers[_pid];
    }

    // Admin functions
    function updatePool(
        uint256 _pid,
        uint256 _rewardPerSecond,
        uint256 _allocPoint,
        uint256 _endTime
    ) external onlyRole(FARM_MANAGER_ROLE) {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        
        _updateAllPools();
        
        PoolInfo storage pool = poolInfo[_pid];
        totalAllocPoint = totalAllocPoint - pool.allocPoint + _allocPoint;
        
        pool.rewardPerSecond = _rewardPerSecond;
        pool.allocPoint = _allocPoint;
        
        if (_endTime > 0 && _endTime > block.timestamp) {
            pool.endTime = _endTime;
        }
        
        emit PoolUpdated(_pid, _rewardPerSecond, _allocPoint);
    }

    function addBoostTier(
        uint256 _pid,
        uint256 _lockDuration,
        uint256 _multiplier
    ) external onlyRole(FARM_MANAGER_ROLE) {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        require(_multiplier <= MAX_BOOST_MULTIPLIER, "YieldFarming: multiplier too high");
        
        _addBoostTier(_pid, _lockDuration, _multiplier);
        emit BoostTierAdded(_pid, _lockDuration, _multiplier);
    }

    function setPoolActive(uint256 _pid, bool _isActive) external onlyRole(FARM_MANAGER_ROLE) {
        require(_pid < poolInfo.length, "YieldFarming: invalid pool");
        poolInfo[_pid].isActive = _isActive;
    }

    function setAuthorizedCompounder(address _compounder, bool _authorized) external onlyRole(ADMIN_ROLE) {
        authorizedCompounders[_compounder] = _authorized;
    }

    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    function emergencyRewardWithdraw(uint256 _pid, uint256 _amount) external onlyRole(ADMIN_ROLE) {
        PoolInfo storage pool = poolInfo[_pid];
        pool.rewardToken.safeTransfer(treasury, _amount);
    }
}