// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";

/**
 * @title SimpleToken
 * @dev A basic ERC20 token implementation for testing and demonstration
 * Features:
 * - Standard ERC20 functionality
 * - Owner-only minting capability
 * - Initial supply of 1,000,000 tokens
 */
contract SimpleToken is ERC20 {
    address public owner;
    
    constructor() ERC20("Simple Token", "SIMPLE") {
        owner = msg.sender;
        _mint(msg.sender, 1000000 * 10**decimals());
    }
    
    /**
     * @dev Mint new tokens (only owner)
     * @param to Address to mint tokens to
     * @param amount Amount of tokens to mint
     */
    function mint(address to, uint256 amount) public {
        require(msg.sender == owner, "Only owner can mint");
        _mint(to, amount);
    }
}