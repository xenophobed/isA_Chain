// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/governance/Governor.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorSettings.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorCountingSimple.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorVotes.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorVotesQuorumFraction.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorTimelockControl.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";

/**
 * @title ISAGovernor
 * @dev Governance contract for the isA_Chain ecosystem
 * 
 * Features:
 * - Proposal creation and voting
 * - Timelock execution for security
 * - Quorum-based governance
 * - Role-based proposal management
 * - Emergency governance procedures
 */
contract ISAGovernor is
    Governor,
    GovernorSettings,
    GovernorCountingSimple,
    GovernorVotes,
    GovernorVotesQuorumFraction,
    GovernorTimelockControl,
    AccessControl,
    ReentrancyGuard
{
    // Roles
    bytes32 public constant PROPOSER_ROLE = keccak256("PROPOSER_ROLE");
    bytes32 public constant EMERGENCY_ROLE = keccak256("EMERGENCY_ROLE");
    bytes32 public constant GUARDIAN_ROLE = keccak256("GUARDIAN_ROLE");
    
    // Governance parameters
    uint256 public constant MIN_PROPOSAL_THRESHOLD = 1000000 * 10**18; // 1M ISA
    uint256 public constant MAX_PROPOSAL_THRESHOLD = 10000000 * 10**18; // 10M ISA
    
    // Emergency governance
    mapping(uint256 => bool) public emergencyProposals;
    uint256 public emergencyVotingPeriod = 1 days;
    uint256 public emergencyQuorum = 3000; // 30%
    
    // Proposal categories
    enum ProposalCategory {
        STANDARD,
        PARAMETER_CHANGE,
        TREASURY,
        EMERGENCY,
        UPGRADE
    }
    
    mapping(uint256 => ProposalCategory) public proposalCategories;
    mapping(ProposalCategory => uint256) public categoryQuorums;
    
    // Events
    event EmergencyProposalCreated(uint256 indexed proposalId, address indexed proposer);
    event EmergencyVotingPeriodUpdated(uint256 oldPeriod, uint256 newPeriod);
    event ProposalCategorized(uint256 indexed proposalId, ProposalCategory category);
    event GuardianAction(address indexed guardian, uint256 indexed proposalId, string action);
    
    /**
     * @dev Constructor
     * @param _token The voting token contract
     * @param _timelock The timelock controller contract
     */
    constructor(
        IVotes _token,
        TimelockController _timelock
    )
        Governor("isA Chain Governor")
        GovernorSettings(1 days, 1 weeks, MIN_PROPOSAL_THRESHOLD)
        GovernorVotes(_token)
        GovernorVotesQuorumFraction(4) // 4% quorum
        GovernorTimelockControl(_timelock)
    {
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(PROPOSER_ROLE, msg.sender);
        _grantRole(EMERGENCY_ROLE, msg.sender);
        _grantRole(GUARDIAN_ROLE, msg.sender);
        
        // Set category quorums
        categoryQuorums[ProposalCategory.STANDARD] = 400; // 4%
        categoryQuorums[ProposalCategory.PARAMETER_CHANGE] = 600; // 6%
        categoryQuorums[ProposalCategory.TREASURY] = 800; // 8%
        categoryQuorums[ProposalCategory.EMERGENCY] = 3000; // 30%
        categoryQuorums[ProposalCategory.UPGRADE] = 1000; // 10%
    }
    
    /**
     * @dev Create a standard proposal
     */
    function propose(
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        string memory description
    ) public virtual override(Governor) returns (uint256) {
        require(hasRole(PROPOSER_ROLE, msg.sender) || getVotes(msg.sender, block.number - 1) >= proposalThreshold(), "ISAGovernor: insufficient proposal power");
        
        uint256 proposalId = super.propose(targets, values, calldatas, description);
        proposalCategories[proposalId] = ProposalCategory.STANDARD;
        
        emit ProposalCategorized(proposalId, ProposalCategory.STANDARD);
        return proposalId;
    }
    
    /**
     * @dev Create an emergency proposal
     */
    function proposeEmergency(
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        string memory description
    ) external onlyRole(EMERGENCY_ROLE) nonReentrant returns (uint256) {
        uint256 proposalId = super.propose(targets, values, calldatas, description);
        
        emergencyProposals[proposalId] = true;
        proposalCategories[proposalId] = ProposalCategory.EMERGENCY;
        
        emit EmergencyProposalCreated(proposalId, msg.sender);
        emit ProposalCategorized(proposalId, ProposalCategory.EMERGENCY);
        
        return proposalId;
    }
    
    /**
     * @dev Create a categorized proposal
     */
    function proposeCategorized(
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        string memory description,
        ProposalCategory category
    ) external returns (uint256) {
        require(hasRole(PROPOSER_ROLE, msg.sender) || getVotes(msg.sender, block.number - 1) >= proposalThreshold(), "ISAGovernor: insufficient proposal power");
        require(category != ProposalCategory.EMERGENCY, "ISAGovernor: use proposeEmergency for emergency proposals");
        
        uint256 proposalId = super.propose(targets, values, calldatas, description);
        proposalCategories[proposalId] = category;
        
        emit ProposalCategorized(proposalId, category);
        return proposalId;
    }
    
    /**
     * @dev Guardian veto power for emergency situations
     */
    function guardianVeto(uint256 proposalId) external onlyRole(GUARDIAN_ROLE) {
        require(state(proposalId) == ProposalState.Pending || state(proposalId) == ProposalState.Active, "ISAGovernor: proposal not active");
        
        _cancel(
            _getTargets(proposalId),
            _getValues(proposalId),
            _getCalldatas(proposalId),
            keccak256(bytes(_getDescription(proposalId)))
        );
        
        emit GuardianAction(msg.sender, proposalId, "veto");
    }
    
    /**
     * @dev Update emergency voting period
     */
    function updateEmergencyVotingPeriod(uint256 newPeriod) external onlyGovernance {
        require(newPeriod >= 1 hours && newPeriod <= 7 days, "ISAGovernor: invalid emergency voting period");
        
        uint256 oldPeriod = emergencyVotingPeriod;
        emergencyVotingPeriod = newPeriod;
        
        emit EmergencyVotingPeriodUpdated(oldPeriod, newPeriod);
    }
    
    /**
     * @dev Update category quorum
     */
    function updateCategoryQuorum(ProposalCategory category, uint256 newQuorum) external onlyGovernance {
        require(newQuorum <= 5000, "ISAGovernor: quorum too high"); // Max 50%
        categoryQuorums[category] = newQuorum;
    }
    
    /**
     * @dev Get voting period for proposal
     */
    function votingPeriod() public view virtual override(IGovernor, GovernorSettings) returns (uint256) {
        return super.votingPeriod();
    }
    
    /**
     * @dev Get voting period for specific proposal
     */
    function proposalVotingPeriod(uint256 proposalId) public view returns (uint256) {
        if (emergencyProposals[proposalId]) {
            return emergencyVotingPeriod;
        }
        return votingPeriod();
    }
    
    /**
     * @dev Get quorum for proposal based on category
     */
    function quorum(uint256 blockNumber) public view virtual override(IGovernor, GovernorVotesQuorumFraction) returns (uint256) {
        return super.quorum(blockNumber);
    }
    
    /**
     * @dev Get quorum for specific proposal category
     */
    function proposalQuorum(uint256 proposalId, uint256 blockNumber) public view returns (uint256) {
        ProposalCategory category = proposalCategories[proposalId];
        uint256 categoryQuorum = categoryQuorums[category];
        
        return (token.getPastTotalSupply(blockNumber) * categoryQuorum) / 10000;
    }
    
    /**
     * @dev Check if proposal has reached quorum
     */
    function _quorumReached(uint256 proposalId) internal view virtual override returns (bool) {
        (uint256 againstVotes, uint256 forVotes, uint256 abstainVotes) = proposalVotes(proposalId);
        uint256 snapshot = proposalSnapshot(proposalId);
        
        if (emergencyProposals[proposalId]) {
            uint256 emergencyQuorumVotes = (token.getPastTotalSupply(snapshot) * emergencyQuorum) / 10000;
            return forVotes + abstainVotes >= emergencyQuorumVotes;
        }
        
        return forVotes + abstainVotes >= proposalQuorum(proposalId, snapshot);
    }
    
    /**
     * @dev Get proposal state with emergency handling
     */
    function state(uint256 proposalId) public view virtual override(GovernorTimelockControl) returns (ProposalState) {
        ProposalState currentState = super.state(proposalId);
        
        // Handle emergency proposal timing
        if (emergencyProposals[proposalId] && currentState == ProposalState.Active) {
            uint256 deadline = proposalDeadline(proposalId);
            if (block.number > deadline) {
                return _quorumReached(proposalId) && _voteSucceeded(proposalId) 
                    ? ProposalState.Succeeded 
                    : ProposalState.Defeated;
            }
        }
        
        return currentState;
    }
    
    // Required overrides
    function proposalThreshold() public view virtual override(GovernorSettings) returns (uint256) {
        return super.proposalThreshold();
    }
    
    function _execute(
        uint256 proposalId,
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        bytes32 descriptionHash
    ) internal virtual override(GovernorTimelockControl) {
        super._execute(proposalId, targets, values, calldatas, descriptionHash);
    }
    
    function _cancel(
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        bytes32 descriptionHash
    ) internal virtual override(GovernorTimelockControl) returns (uint256) {
        return super._cancel(targets, values, calldatas, descriptionHash);
    }

    /**
     * @dev Execute operations - required override for multiple inheritance
     */
    function _executeOperations(
        uint256 proposalId,
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        bytes32 descriptionHash
    ) internal virtual override(GovernorTimelockControl) {
        super._executeOperations(proposalId, targets, values, calldatas, descriptionHash);
    }

    /**
     * @dev Queue operations - required override for multiple inheritance
     */
    function _queueOperations(
        uint256 proposalId,
        address[] memory targets,
        uint256[] memory values,
        bytes[] memory calldatas,
        bytes32 descriptionHash
    ) internal virtual override(GovernorTimelockControl) returns (uint48) {
        return super._queueOperations(proposalId, targets, values, calldatas, descriptionHash);
    }

    /**
     * @dev Check if proposal needs queuing - required override for multiple inheritance
     */
    function proposalNeedsQueuing(uint256 proposalId) public view virtual override(GovernorTimelockControl) returns (bool) {
        return super.proposalNeedsQueuing(proposalId);
    }
    
    function _executor() internal view virtual override(GovernorTimelockControl) returns (address) {
        return super._executor();
    }
    
    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(GovernorTimelockControl, AccessControl)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
    
    // Helper functions to access internal proposal data
    function _getTargets(uint256 proposalId) internal view returns (address[] memory) {
        // This would need to be implemented based on internal proposal storage
        // For now, returning empty array as placeholder
        return new address[](0);
    }
    
    function _getValues(uint256 proposalId) internal view returns (uint256[] memory) {
        // This would need to be implemented based on internal proposal storage
        return new uint256[](0);
    }
    
    function _getCalldatas(uint256 proposalId) internal view returns (bytes[] memory) {
        // This would need to be implemented based on internal proposal storage
        return new bytes[](0);
    }
    
    function _getDescription(uint256 proposalId) internal view returns (string memory) {
        // This would need to be implemented based on internal proposal storage
        return "";
    }
}