// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Pausable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Votes.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";

/**
 * @title ISAToken
 * @dev The native token of the isA_Chain ecosystem
 * 
 * Features:
 * - ERC20 standard compliance
 * - Governance voting capabilities
 * - Pausable for emergency situations
 * - Burnable tokens
 * - Role-based access control
 * - Permit functionality for gasless approvals
 * - Anti-reentrancy protection
 */
contract ISAToken is 
    ERC20,
    ERC20Burnable,
    ERC20Pausable,
    AccessControl,
    ERC20Permit,
    ERC20Votes,
    ReentrancyGuard
{
    // Roles
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant BURNER_ROLE = keccak256("BURNER_ROLE");
    
    // Token configuration
    uint256 public constant INITIAL_SUPPLY = 1_000_000_000 * 10**18; // 1 billion ISA
    uint256 public constant MAX_SUPPLY = 10_000_000_000 * 10**18; // 10 billion ISA max
    
    // Vesting and distribution
    mapping(address => VestingSchedule) private _vestingSchedules;
    uint256 public totalVested;
    
    struct VestingSchedule {
        uint256 totalAmount;
        uint256 released;
        uint256 start;
        uint256 duration;
        uint256 cliffDuration;
        bool revokable;
        bool revoked;
    }
    
    // Events
    event VestingScheduleCreated(
        address indexed beneficiary,
        uint256 totalAmount,
        uint256 start,
        uint256 duration,
        uint256 cliffDuration
    );
    
    event TokensReleased(address indexed beneficiary, uint256 amount);
    event VestingRevoked(address indexed beneficiary, uint256 unvestedAmount);
    
    /**
     * @dev Constructor
     * @param treasury Address to receive initial token supply
     */
    constructor(address treasury) 
        ERC20("isA Chain Token", "ISA")
        ERC20Permit("isA Chain Token")
    {
        require(treasury != address(0), "ISAToken: treasury cannot be zero address");
        
        // Setup roles
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(MINTER_ROLE, msg.sender);
        _grantRole(PAUSER_ROLE, msg.sender);
        _grantRole(BURNER_ROLE, msg.sender);
        
        // Mint initial supply to treasury
        _mint(treasury, INITIAL_SUPPLY);
    }
    
    /**
     * @dev Mint tokens to specified address
     * @param to Address to mint tokens to
     * @param amount Amount of tokens to mint
     */
    function mint(address to, uint256 amount) 
        external 
        onlyRole(MINTER_ROLE) 
        nonReentrant
    {
        require(totalSupply() + amount <= MAX_SUPPLY, "ISAToken: exceeds max supply");
        _mint(to, amount);
    }
    
    /**
     * @dev Pause token transfers
     */
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }
    
    /**
     * @dev Unpause token transfers
     */
    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
    
    /**
     * @dev Burn tokens from specified address
     * @param from Address to burn tokens from
     * @param amount Amount of tokens to burn
     */
    function burnFrom(address from, uint256 amount) 
        public 
        override
        onlyRole(BURNER_ROLE)
        nonReentrant
    {
        super.burnFrom(from, amount);
    }
    
    /**
     * @dev Create vesting schedule for beneficiary
     * @param beneficiary Address of the beneficiary
     * @param totalAmount Total amount of tokens to vest
     * @param start Start timestamp of vesting
     * @param duration Total duration of vesting in seconds
     * @param cliffDuration Cliff duration in seconds
     * @param revokable Whether the vesting is revokable
     */
    function createVestingSchedule(
        address beneficiary,
        uint256 totalAmount,
        uint256 start,
        uint256 duration,
        uint256 cliffDuration,
        bool revokable
    ) external onlyRole(DEFAULT_ADMIN_ROLE) nonReentrant {
        require(beneficiary != address(0), "ISAToken: beneficiary cannot be zero address");
        require(totalAmount > 0, "ISAToken: total amount must be positive");
        require(duration > 0, "ISAToken: duration must be positive");
        require(cliffDuration <= duration, "ISAToken: cliff duration exceeds total duration");
        require(_vestingSchedules[beneficiary].totalAmount == 0, "ISAToken: vesting schedule already exists");
        
        _vestingSchedules[beneficiary] = VestingSchedule({
            totalAmount: totalAmount,
            released: 0,
            start: start,
            duration: duration,
            cliffDuration: cliffDuration,
            revokable: revokable,
            revoked: false
        });
        
        totalVested += totalAmount;
        
        // Transfer tokens to this contract for vesting
        _transfer(msg.sender, address(this), totalAmount);
        
        emit VestingScheduleCreated(beneficiary, totalAmount, start, duration, cliffDuration);
    }
    
    /**
     * @dev Release vested tokens for beneficiary
     * @param beneficiary Address of the beneficiary
     */
    function release(address beneficiary) external nonReentrant {
        VestingSchedule storage schedule = _vestingSchedules[beneficiary];
        require(schedule.totalAmount > 0, "ISAToken: no vesting schedule");
        require(!schedule.revoked, "ISAToken: vesting schedule revoked");
        
        uint256 releasableAmount = _releasableAmount(beneficiary);
        require(releasableAmount > 0, "ISAToken: no tokens to release");
        
        schedule.released += releasableAmount;
        _transfer(address(this), beneficiary, releasableAmount);
        
        emit TokensReleased(beneficiary, releasableAmount);
    }
    
    /**
     * @dev Revoke vesting schedule
     * @param beneficiary Address of the beneficiary
     */
    function revokeVesting(address beneficiary) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
        nonReentrant 
    {
        VestingSchedule storage schedule = _vestingSchedules[beneficiary];
        require(schedule.totalAmount > 0, "ISAToken: no vesting schedule");
        require(schedule.revokable, "ISAToken: vesting schedule not revokable");
        require(!schedule.revoked, "ISAToken: vesting schedule already revoked");
        
        uint256 releasableAmount = _releasableAmount(beneficiary);
        if (releasableAmount > 0) {
            schedule.released += releasableAmount;
            _transfer(address(this), beneficiary, releasableAmount);
        }
        
        uint256 unvestedAmount = schedule.totalAmount - schedule.released;
        schedule.revoked = true;
        totalVested -= unvestedAmount;
        
        // Return unvested tokens to admin
        _transfer(address(this), msg.sender, unvestedAmount);
        
        emit VestingRevoked(beneficiary, unvestedAmount);
    }
    
    /**
     * @dev Get vesting schedule for beneficiary
     * @param beneficiary Address of the beneficiary
     * @return VestingSchedule struct
     */
    function getVestingSchedule(address beneficiary) 
        external 
        view 
        returns (VestingSchedule memory) 
    {
        return _vestingSchedules[beneficiary];
    }
    
    /**
     * @dev Get releasable amount for beneficiary
     * @param beneficiary Address of the beneficiary
     * @return Amount of tokens that can be released
     */
    function releasableAmount(address beneficiary) external view returns (uint256) {
        return _releasableAmount(beneficiary);
    }
    
    /**
     * @dev Calculate releasable amount for beneficiary
     * @param beneficiary Address of the beneficiary
     * @return Amount of tokens that can be released
     */
    function _releasableAmount(address beneficiary) private view returns (uint256) {
        VestingSchedule storage schedule = _vestingSchedules[beneficiary];
        
        if (schedule.totalAmount == 0 || schedule.revoked) {
            return 0;
        }
        
        if (block.timestamp < schedule.start + schedule.cliffDuration) {
            return 0;
        }
        
        if (block.timestamp >= schedule.start + schedule.duration) {
            return schedule.totalAmount - schedule.released;
        }
        
        uint256 elapsed = block.timestamp - schedule.start;
        uint256 vestedAmount = (schedule.totalAmount * elapsed) / schedule.duration;
        
        return vestedAmount - schedule.released;
    }
    
    // Required overrides
    function _update(address from, address to, uint256 value)
        internal
        override(ERC20, ERC20Pausable, ERC20Votes)
    {
        super._update(from, to, value);
    }
    
    function nonces(address owner)
        public
        view
        override(ERC20Permit, Nonces)
        returns (uint256)
    {
        return super.nonces(owner);
    }
    
    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(AccessControl)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}