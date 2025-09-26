// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";
import "@openzeppelin/contracts/utils/math/Math.sol";

/**
 * @title StakingPool
 * @dev Advanced staking pool with multiple reward tokens and flexible terms
 * 
 * Features:
 * - Multi-token reward distribution
 * - Flexible staking periods with different APY rates
 * - Penalty system for early withdrawal
 * - Auto-compounding option
 * - Emergency withdrawal
 * - Stake delegation
 */
contract StakingPool is Ownable, ReentrancyGuard, Pausable {
    using SafeERC20 for IERC20;
    using Math for uint256;
    
    // Staking token
    IERC20 public immutable stakingToken;
    
    // Reward tokens
    address[] public rewardTokens;
    mapping(address => bool) public isRewardToken;
    mapping(address => uint256) public rewardRates; // Rewards per second per token
    mapping(address => uint256) public lastUpdateTime;
    mapping(address => uint256) public rewardPerTokenStored;
    
    // User staking information
    struct StakeInfo {
        uint256 amount;
        uint256 lockEndTime;
        uint256 stakingPeriod; // in seconds
        uint256 multiplier; // basis points (10000 = 1x)
        bool autoCompound;
        address delegatedTo;
        uint256 stakedAt;
    }
    
    // User reward tracking
    struct UserRewards {
        mapping(address => uint256) userRewardPerTokenPaid;
        mapping(address => uint256) rewards;
    }
    
    mapping(address => StakeInfo[]) public userStakes;
    mapping(address => UserRewards) private userRewards;
    mapping(address => uint256) public totalStakedByUser;
    
    // Delegation tracking
    mapping(address => address[]) public delegators; // delegatee => delegators
    mapping(address => uint256) public delegatedPower; // total delegated voting power
    
    // Pool statistics
    uint256 public totalStaked;
    uint256 public totalRewardsDistributed;
    mapping(address => uint256) public totalRewardsByToken;
    
    // Staking periods and multipliers
    struct StakingTier {
        uint256 minPeriod; // minimum staking period in seconds
        uint256 maxPeriod; // maximum staking period in seconds  
        uint256 multiplier; // APY multiplier in basis points
        uint256 earlyWithdrawalPenalty; // penalty rate in basis points
    }
    
    mapping(uint256 => StakingTier) public stakingTiers;
    uint256 public tierCount;
    
    // Pool configuration
    uint256 public constant MAX_STAKING_PERIOD = 365 days;
    uint256 public constant MIN_STAKING_PERIOD = 1 days;
    uint256 public constant MAX_PENALTY_RATE = 2500; // 25%
    uint256 public minStakeAmount = 1e18; // 1 token minimum
    uint256 public maxStakeAmount = 1000000e18; // 1M tokens maximum
    
    // Emergency settings
    bool public emergencyWithdrawEnabled = false;
    uint256 public emergencyWithdrawPenalty = 1000; // 10%
    address public penaltyRecipient;
    
    // Events
    event Staked(
        address indexed user, 
        uint256 indexed stakeId,
        uint256 amount, 
        uint256 lockPeriod,
        uint256 multiplier,
        bool autoCompound
    );
    
    event Unstaked(
        address indexed user,
        uint256 indexed stakeId,
        uint256 amount,
        uint256 penalty
    );
    
    event RewardsClaimed(
        address indexed user,
        address indexed rewardToken,
        uint256 amount
    );
    
    event RewardAdded(
        address indexed rewardToken,
        uint256 reward
    );
    
    event StakeDelegated(
        address indexed delegator,
        address indexed delegatee,
        uint256 indexed stakeId
    );
    
    event TierAdded(
        uint256 indexed tierId,
        uint256 minPeriod,
        uint256 maxPeriod,
        uint256 multiplier,
        uint256 penalty
    );
    
    event EmergencyWithdraw(
        address indexed user,
        uint256 amount,
        uint256 penalty
    );
    
    /**
     * @dev Constructor
     * @param _stakingToken Address of the token to be staked
     * @param _penaltyRecipient Address to receive withdrawal penalties
     */
    constructor(
        address _stakingToken,
        address _penaltyRecipient
    ) Ownable(msg.sender) {
        require(_stakingToken != address(0), "StakingPool: invalid staking token");
        require(_penaltyRecipient != address(0), "StakingPool: invalid penalty recipient");
        
        stakingToken = IERC20(_stakingToken);
        penaltyRecipient = _penaltyRecipient;
        
        // Add default staking tiers
        _addTier(1 days, 30 days, 10000, 500); // 1-30 days, 1x multiplier, 5% penalty
        _addTier(30 days, 90 days, 12000, 300); // 30-90 days, 1.2x multiplier, 3% penalty
        _addTier(90 days, 180 days, 15000, 200); // 90-180 days, 1.5x multiplier, 2% penalty
        _addTier(180 days, 365 days, 20000, 100); // 180-365 days, 2x multiplier, 1% penalty
    }
    
    /**
     * @dev Add a new reward token
     * @param rewardToken Address of the reward token
     * @param rewardRate Reward rate per second
     */
    function addRewardToken(address rewardToken, uint256 rewardRate) 
        external 
        onlyOwner 
    {
        require(rewardToken != address(0), "StakingPool: invalid reward token");
        require(!isRewardToken[rewardToken], "StakingPool: reward token already added");
        require(rewardRate > 0, "StakingPool: reward rate must be positive");
        
        rewardTokens.push(rewardToken);
        isRewardToken[rewardToken] = true;
        rewardRates[rewardToken] = rewardRate;
        lastUpdateTime[rewardToken] = block.timestamp;
        
        emit RewardAdded(rewardToken, 0);
    }
    
    /**
     * @dev Update reward rate for a token
     * @param rewardToken Address of the reward token
     * @param newRate New reward rate per second
     */
    function updateRewardRate(address rewardToken, uint256 newRate) 
        external 
        onlyOwner 
    {
        require(isRewardToken[rewardToken], "StakingPool: not a reward token");
        
        _updateReward(address(0), rewardToken);
        rewardRates[rewardToken] = newRate;
    }
    
    /**
     * @dev Stake tokens with specified parameters
     * @param amount Amount to stake
     * @param stakingPeriod Staking period in seconds
     * @param autoCompound Whether to auto-compound rewards
     * @param delegateTo Address to delegate voting power to (optional)
     */
    function stake(
        uint256 amount,
        uint256 stakingPeriod,
        bool autoCompound,
        address delegateTo
    ) external whenNotPaused nonReentrant returns (uint256 stakeId) {
        require(amount >= minStakeAmount, "StakingPool: amount below minimum");
        require(amount <= maxStakeAmount, "StakingPool: amount exceeds maximum");
        require(stakingPeriod >= MIN_STAKING_PERIOD, "StakingPool: staking period too short");
        require(stakingPeriod <= MAX_STAKING_PERIOD, "StakingPool: staking period too long");
        
        // Find appropriate tier
        uint256 tierId = _findTierForPeriod(stakingPeriod);
        require(tierId < tierCount, "StakingPool: invalid staking period");
        
        StakingTier memory tier = stakingTiers[tierId];
        
        // Update rewards for all tokens
        for (uint256 i = 0; i < rewardTokens.length; i++) {
            _updateReward(msg.sender, rewardTokens[i]);
        }
        
        // Create stake info
        stakeId = userStakes[msg.sender].length;
        userStakes[msg.sender].push(StakeInfo({
            amount: amount,
            lockEndTime: block.timestamp + stakingPeriod,
            stakingPeriod: stakingPeriod,
            multiplier: tier.multiplier,
            autoCompound: autoCompound,
            delegatedTo: delegateTo,
            stakedAt: block.timestamp
        }));
        
        // Update totals
        totalStaked += amount;
        totalStakedByUser[msg.sender] += amount;
        
        // Handle delegation
        if (delegateTo != address(0) && delegateTo != msg.sender) {
            delegators[delegateTo].push(msg.sender);
            delegatedPower[delegateTo] += amount;
        }
        
        // Transfer tokens
        stakingToken.safeTransferFrom(msg.sender, address(this), amount);
        
        emit Staked(msg.sender, stakeId, amount, stakingPeriod, tier.multiplier, autoCompound);
        
        if (delegateTo != address(0) && delegateTo != msg.sender) {
            emit StakeDelegated(msg.sender, delegateTo, stakeId);
        }
        
        return stakeId;
    }
    
    /**
     * @dev Unstake tokens
     * @param stakeId ID of the stake to unstake
     */
    function unstake(uint256 stakeId) external nonReentrant {
        require(stakeId < userStakes[msg.sender].length, "StakingPool: invalid stake ID");
        
        StakeInfo storage stakeInfo = userStakes[msg.sender][stakeId];
        require(stakeInfo.amount > 0, "StakingPool: stake already withdrawn");
        
        // Update rewards for all tokens
        for (uint256 i = 0; i < rewardTokens.length; i++) {
            _updateReward(msg.sender, rewardTokens[i]);
        }
        
        uint256 amount = stakeInfo.amount;
        uint256 penalty = 0;
        
        // Calculate penalty if unstaking early
        if (block.timestamp < stakeInfo.lockEndTime) {
            uint256 tierId = _findTierForPeriod(stakeInfo.stakingPeriod);
            if (tierId < tierCount) {
                penalty = (amount * stakingTiers[tierId].earlyWithdrawalPenalty) / 10000;
            }
        }
        
        // Update delegation
        if (stakeInfo.delegatedTo != address(0) && stakeInfo.delegatedTo != msg.sender) {
            delegatedPower[stakeInfo.delegatedTo] -= amount;
            _removeDelegator(stakeInfo.delegatedTo, msg.sender);
        }
        
        // Update totals
        totalStaked -= amount;
        totalStakedByUser[msg.sender] -= amount;
        
        // Clear stake info
        stakeInfo.amount = 0;
        
        // Transfer tokens (minus penalty)
        uint256 withdrawAmount = amount - penalty;
        if (withdrawAmount > 0) {
            stakingToken.safeTransfer(msg.sender, withdrawAmount);
        }
        
        // Send penalty to recipient
        if (penalty > 0) {
            stakingToken.safeTransfer(penaltyRecipient, penalty);
        }
        
        emit Unstaked(msg.sender, stakeId, amount, penalty);
    }
    
    /**
     * @dev Claim all pending rewards
     */
    function claimRewards() external nonReentrant {
        for (uint256 i = 0; i < rewardTokens.length; i++) {
            address rewardToken = rewardTokens[i];
            _updateReward(msg.sender, rewardToken);
            
            uint256 reward = userRewards[msg.sender].rewards[rewardToken];
            if (reward > 0) {
                userRewards[msg.sender].rewards[rewardToken] = 0;
                totalRewardsDistributed += reward;
                totalRewardsByToken[rewardToken] += reward;
                
                IERC20(rewardToken).safeTransfer(msg.sender, reward);
                emit RewardsClaimed(msg.sender, rewardToken, reward);
            }
        }
    }
    
    /**
     * @dev Claim rewards for specific token
     * @param rewardToken Address of the reward token to claim
     */
    function claimReward(address rewardToken) external nonReentrant {
        require(isRewardToken[rewardToken], "StakingPool: not a reward token");
        
        _updateReward(msg.sender, rewardToken);
        
        uint256 reward = userRewards[msg.sender].rewards[rewardToken];
        if (reward > 0) {
            userRewards[msg.sender].rewards[rewardToken] = 0;
            totalRewardsDistributed += reward;
            totalRewardsByToken[rewardToken] += reward;
            
            IERC20(rewardToken).safeTransfer(msg.sender, reward);
            emit RewardsClaimed(msg.sender, rewardToken, reward);
        }
    }
    
    /**
     * @dev Emergency withdraw all stakes (with penalty)
     */
    function emergencyWithdraw() external nonReentrant {
        require(emergencyWithdrawEnabled, "StakingPool: emergency withdraw not enabled");
        
        uint256 totalAmount = totalStakedByUser[msg.sender];
        require(totalAmount > 0, "StakingPool: no stakes to withdraw");
        
        // Calculate penalty
        uint256 penalty = (totalAmount * emergencyWithdrawPenalty) / 10000;
        uint256 withdrawAmount = totalAmount - penalty;
        
        // Clear all user stakes
        delete userStakes[msg.sender];
        totalStaked -= totalAmount;
        totalStakedByUser[msg.sender] = 0;
        
        // Transfer tokens
        if (withdrawAmount > 0) {
            stakingToken.safeTransfer(msg.sender, withdrawAmount);
        }
        
        if (penalty > 0) {
            stakingToken.safeTransfer(penaltyRecipient, penalty);
        }
        
        emit EmergencyWithdraw(msg.sender, totalAmount, penalty);
    }
    
    /**
     * @dev Get user's total staked amount with multipliers
     * @param user Address of the user
     * @return Total effective staked amount
     */
    function getEffectiveStakedAmount(address user) external view returns (uint256) {
        uint256 effectiveAmount = 0;
        StakeInfo[] memory stakes = userStakes[user];
        
        for (uint256 i = 0; i < stakes.length; i++) {
            if (stakes[i].amount > 0) {
                effectiveAmount += (stakes[i].amount * stakes[i].multiplier) / 10000;
            }
        }
        
        return effectiveAmount;
    }
    
    /**
     * @dev Get user's pending rewards for all tokens
     * @param user Address of the user
     * @return tokens Array of reward token addresses
     * @return amounts Array of pending reward amounts
     */
    function getPendingRewards(address user) 
        external 
        view 
        returns (address[] memory tokens, uint256[] memory amounts) 
    {
        tokens = new address[](rewardTokens.length);
        amounts = new uint256[](rewardTokens.length);
        
        for (uint256 i = 0; i < rewardTokens.length; i++) {
            address token = rewardTokens[i];
            tokens[i] = token;
            amounts[i] = _calculatePendingReward(user, token);
        }
        
        return (tokens, amounts);
    }
    
    /**
     * @dev Get user's stake information
     * @param user Address of the user
     * @return Array of stake info
     */
    function getUserStakes(address user) external view returns (StakeInfo[] memory) {
        return userStakes[user];
    }
    
    /**
     * @dev Get user's voting power (including delegated)
     * @param user Address of the user
     * @return Total voting power
     */
    function getVotingPower(address user) external view returns (uint256) {
        return totalStakedByUser[user] + delegatedPower[user];
    }
    
    /**
     * @dev Add a new staking tier
     * @param minPeriod Minimum staking period
     * @param maxPeriod Maximum staking period
     * @param multiplier APY multiplier in basis points
     * @param penalty Early withdrawal penalty in basis points
     */
    function addTier(
        uint256 minPeriod,
        uint256 maxPeriod, 
        uint256 multiplier,
        uint256 penalty
    ) external onlyOwner {
        _addTier(minPeriod, maxPeriod, multiplier, penalty);
    }
    
    /**
     * @dev Update staking limits
     * @param newMinStake New minimum stake amount
     * @param newMaxStake New maximum stake amount
     */
    function updateStakingLimits(uint256 newMinStake, uint256 newMaxStake) 
        external 
        onlyOwner 
    {
        require(newMinStake > 0, "StakingPool: min stake must be positive");
        require(newMaxStake > newMinStake, "StakingPool: max stake must be greater than min");
        
        minStakeAmount = newMinStake;
        maxStakeAmount = newMaxStake;
    }
    
    /**
     * @dev Toggle emergency withdraw
     * @param enabled Whether to enable emergency withdraw
     */
    function setEmergencyWithdraw(bool enabled) external onlyOwner {
        emergencyWithdrawEnabled = enabled;
    }
    
    /**
     * @dev Update emergency withdraw penalty
     * @param newPenalty New penalty rate in basis points
     */
    function setEmergencyWithdrawPenalty(uint256 newPenalty) external onlyOwner {
        require(newPenalty <= MAX_PENALTY_RATE, "StakingPool: penalty rate too high");
        emergencyWithdrawPenalty = newPenalty;
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
    
    function _addTier(
        uint256 minPeriod,
        uint256 maxPeriod,
        uint256 multiplier,
        uint256 penalty
    ) internal {
        require(minPeriod >= MIN_STAKING_PERIOD, "StakingPool: min period too short");
        require(maxPeriod <= MAX_STAKING_PERIOD, "StakingPool: max period too long");
        require(minPeriod <= maxPeriod, "StakingPool: invalid period range");
        require(multiplier >= 10000, "StakingPool: multiplier must be at least 1x");
        require(penalty <= MAX_PENALTY_RATE, "StakingPool: penalty rate too high");
        
        stakingTiers[tierCount] = StakingTier({
            minPeriod: minPeriod,
            maxPeriod: maxPeriod,
            multiplier: multiplier,
            earlyWithdrawalPenalty: penalty
        });
        
        emit TierAdded(tierCount, minPeriod, maxPeriod, multiplier, penalty);
        tierCount++;
    }
    
    function _findTierForPeriod(uint256 period) internal view returns (uint256) {
        for (uint256 i = 0; i < tierCount; i++) {
            StakingTier memory tier = stakingTiers[i];
            if (period >= tier.minPeriod && period <= tier.maxPeriod) {
                return i;
            }
        }
        return tierCount; // Invalid tier
    }
    
    function _updateReward(address account, address rewardToken) internal {
        rewardPerTokenStored[rewardToken] = _rewardPerToken(rewardToken);
        lastUpdateTime[rewardToken] = block.timestamp;
        
        if (account != address(0)) {
            userRewards[account].rewards[rewardToken] = _calculatePendingReward(account, rewardToken);
            userRewards[account].userRewardPerTokenPaid[rewardToken] = rewardPerTokenStored[rewardToken];
        }
    }
    
    function _rewardPerToken(address rewardToken) internal view returns (uint256) {
        if (totalStaked == 0) {
            return rewardPerTokenStored[rewardToken];
        }
        
        return rewardPerTokenStored[rewardToken] + 
            (((block.timestamp - lastUpdateTime[rewardToken]) * rewardRates[rewardToken] * 1e18) / totalStaked);
    }
    
    function _calculatePendingReward(address account, address rewardToken) internal view returns (uint256) {
        uint256 effectiveStake = 0;
        StakeInfo[] memory stakes = userStakes[account];
        
        // Calculate effective staking amount with multipliers
        for (uint256 i = 0; i < stakes.length; i++) {
            if (stakes[i].amount > 0) {
                effectiveStake += (stakes[i].amount * stakes[i].multiplier) / 10000;
            }
        }
        
        return ((effectiveStake * (_rewardPerToken(rewardToken) - userRewards[account].userRewardPerTokenPaid[rewardToken])) / 1e18) +
               userRewards[account].rewards[rewardToken];
    }
    
    function _removeDelegator(address delegatee, address delegator) internal {
        address[] storage delegatorsList = delegators[delegatee];
        for (uint256 i = 0; i < delegatorsList.length; i++) {
            if (delegatorsList[i] == delegator) {
                delegatorsList[i] = delegatorsList[delegatorsList.length - 1];
                delegatorsList.pop();
                break;
            }
        }
    }
    
    /**
     * @dev Emergency function to recover stuck ERC20 tokens
     * @param token Token address
     * @param amount Amount to recover
     */
    function emergencyRecoverToken(address token, uint256 amount) external onlyOwner {
        require(token != address(stakingToken) || amount <= IERC20(token).balanceOf(address(this)) - totalStaked, 
                "StakingPool: cannot recover staked tokens");
        
        IERC20(token).safeTransfer(owner(), amount);
    }
}