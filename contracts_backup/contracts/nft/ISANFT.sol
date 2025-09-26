// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721Enumerable.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721Pausable.sol";
import "@openzeppelin/contracts/access/AccessControl.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721Burnable.sol";
import "@openzeppelin/contracts/utils/ReentrancyGuard.sol";
import "@openzeppelin/contracts/token/common/ERC2981.sol";

/**
 * @title ISANFT
 * @dev Advanced NFT contract for the isA_Chain ecosystem
 * 
 * Features:
 * - ERC721 standard compliance with extensions
 * - Enumerable for token discovery
 * - URI storage for metadata
 * - Pausable for emergency situations
 * - Role-based access control
 * - Burnable tokens
 * - Royalty support (ERC2981)
 * - Batch operations
 * - Reveal mechanism
 * - Whitelist minting
 */
contract ISANFT is
    ERC721,
    ERC721Enumerable,
    ERC721URIStorage,
    ERC721Pausable,
    AccessControl,
    ERC721Burnable,
    ERC2981,
    ReentrancyGuard
{
    // Roles
    bytes32 public constant MINTER_ROLE = keccak256("MINTER_ROLE");
    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");
    bytes32 public constant METADATA_ROLE = keccak256("METADATA_ROLE");
    bytes32 public constant ROYALTY_ROLE = keccak256("ROYALTY_ROLE");
    
    // Collection configuration
    uint256 public constant MAX_SUPPLY = 10000;
    uint256 public constant MAX_MINT_PER_TRANSACTION = 10;
    uint256 public constant MAX_MINT_PER_WALLET = 50;
    
    // Minting configuration
    uint256 public mintPrice = 0.05 ether;
    uint256 public whitelistPrice = 0.03 ether;
    bool public publicMintEnabled = false;
    bool public whitelistMintEnabled = false;
    bool public revealed = false;
    
    // Collection metadata
    string private _baseTokenURI;
    string private _hiddenMetadataURI;
    string private _contractURI;
    
    // Tracking
    uint256 private _currentIndex = 1; // Start from token ID 1
    mapping(address => uint256) public walletMints;
    mapping(address => bool) public whitelist;
    uint256 public whitelistCount;
    
    // Royalties
    address public royaltyRecipient;
    uint96 public royaltyFeeBps = 750; // 7.5%
    
    // Withdraw addresses
    address public treasury;
    address public artist;
    uint256 public treasuryShare = 7000; // 70%
    uint256 public artistShare = 3000; // 30%
    
    // Events
    event BatchMinted(address indexed to, uint256[] tokenIds);
    event WhitelistStatusChanged(address indexed user, bool status);
    event RevealStatusChanged(bool revealed);
    event PriceUpdated(uint256 newPrice, uint256 newWhitelistPrice);
    event MintingConfigUpdated(bool publicEnabled, bool whitelistEnabled);
    event RoyaltyUpdated(address recipient, uint96 feeBps);
    event WithdrawSplitUpdated(uint256 treasuryShare, uint256 artistShare);
    
    /**
     * @dev Constructor
     * @param _treasury Treasury address for funds
     * @param _artist Artist address for royalties
     * @param _royaltyRecipient Initial royalty recipient
     */
    constructor(
        address _treasury,
        address _artist,
        address _royaltyRecipient
    ) ERC721("isA Chain NFT", "ISANFT") {
        require(_treasury != address(0), "ISANFT: treasury cannot be zero");
        require(_artist != address(0), "ISANFT: artist cannot be zero");
        require(_royaltyRecipient != address(0), "ISANFT: royalty recipient cannot be zero");
        
        treasury = _treasury;
        artist = _artist;
        royaltyRecipient = _royaltyRecipient;
        
        // Setup roles
        _grantRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _grantRole(MINTER_ROLE, msg.sender);
        _grantRole(PAUSER_ROLE, msg.sender);
        _grantRole(METADATA_ROLE, msg.sender);
        _grantRole(ROYALTY_ROLE, msg.sender);
        
        // Set default royalty
        _setDefaultRoyalty(_royaltyRecipient, royaltyFeeBps);
        
        // Set contract URI
        _contractURI = "https://api.isachain.io/nft/contract-metadata";
    }
    
    /**
     * @dev Public minting function
     * @param quantity Number of tokens to mint
     */
    function mint(uint256 quantity) external payable nonReentrant {
        require(publicMintEnabled, "ISANFT: public minting not enabled");
        require(quantity > 0 && quantity <= MAX_MINT_PER_TRANSACTION, "ISANFT: invalid quantity");
        require(_currentIndex + quantity <= MAX_SUPPLY + 1, "ISANFT: exceeds max supply");
        require(walletMints[msg.sender] + quantity <= MAX_MINT_PER_WALLET, "ISANFT: exceeds wallet limit");
        require(msg.value >= mintPrice * quantity, "ISANFT: insufficient payment");
        
        walletMints[msg.sender] += quantity;
        _batchMint(msg.sender, quantity);
    }
    
    /**
     * @dev Whitelist minting function
     * @param quantity Number of tokens to mint
     */
    function whitelistMint(uint256 quantity) external payable nonReentrant {
        require(whitelistMintEnabled, "ISANFT: whitelist minting not enabled");
        require(whitelist[msg.sender], "ISANFT: not whitelisted");
        require(quantity > 0 && quantity <= MAX_MINT_PER_TRANSACTION, "ISANFT: invalid quantity");
        require(_currentIndex + quantity <= MAX_SUPPLY + 1, "ISANFT: exceeds max supply");
        require(walletMints[msg.sender] + quantity <= MAX_MINT_PER_WALLET, "ISANFT: exceeds wallet limit");
        require(msg.value >= whitelistPrice * quantity, "ISANFT: insufficient payment");
        
        walletMints[msg.sender] += quantity;
        _batchMint(msg.sender, quantity);
    }
    
    /**
     * @dev Admin minting function
     * @param to Address to mint to
     * @param quantity Number of tokens to mint
     */
    function adminMint(address to, uint256 quantity) 
        external 
        onlyRole(MINTER_ROLE) 
        nonReentrant 
    {
        require(to != address(0), "ISANFT: cannot mint to zero address");
        require(quantity > 0, "ISANFT: quantity must be positive");
        require(_currentIndex + quantity <= MAX_SUPPLY + 1, "ISANFT: exceeds max supply");
        
        _batchMint(to, quantity);
    }
    
    /**
     * @dev Batch minting internal function
     * @param to Address to mint to
     * @param quantity Number of tokens to mint
     */
    function _batchMint(address to, uint256 quantity) internal {
        uint256[] memory tokenIds = new uint256[](quantity);
        
        for (uint256 i = 0; i < quantity; i++) {
            uint256 tokenId = _currentIndex;
            tokenIds[i] = tokenId;
            _safeMint(to, tokenId);
            _currentIndex++;
        }
        
        emit BatchMinted(to, tokenIds);
    }
    
    /**
     * @dev Add addresses to whitelist
     * @param addresses Array of addresses to whitelist
     */
    function addToWhitelist(address[] calldata addresses) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        for (uint256 i = 0; i < addresses.length; i++) {
            if (!whitelist[addresses[i]]) {
                whitelist[addresses[i]] = true;
                whitelistCount++;
                emit WhitelistStatusChanged(addresses[i], true);
            }
        }
    }
    
    /**
     * @dev Remove addresses from whitelist
     * @param addresses Array of addresses to remove
     */
    function removeFromWhitelist(address[] calldata addresses) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        for (uint256 i = 0; i < addresses.length; i++) {
            if (whitelist[addresses[i]]) {
                whitelist[addresses[i]] = false;
                whitelistCount--;
                emit WhitelistStatusChanged(addresses[i], false);
            }
        }
    }
    
    /**
     * @dev Set minting prices
     * @param _mintPrice New public mint price
     * @param _whitelistPrice New whitelist mint price
     */
    function setPrices(uint256 _mintPrice, uint256 _whitelistPrice) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        mintPrice = _mintPrice;
        whitelistPrice = _whitelistPrice;
        emit PriceUpdated(_mintPrice, _whitelistPrice);
    }
    
    /**
     * @dev Configure minting settings
     * @param _publicEnabled Enable/disable public minting
     * @param _whitelistEnabled Enable/disable whitelist minting
     */
    function setMintingConfig(bool _publicEnabled, bool _whitelistEnabled) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        publicMintEnabled = _publicEnabled;
        whitelistMintEnabled = _whitelistEnabled;
        emit MintingConfigUpdated(_publicEnabled, _whitelistEnabled);
    }
    
    /**
     * @dev Set base URI for revealed metadata
     * @param newBaseURI New base URI
     */
    function setBaseURI(string calldata newBaseURI) 
        external 
        onlyRole(METADATA_ROLE) 
    {
        _baseTokenURI = newBaseURI;
    }
    
    /**
     * @dev Set hidden metadata URI
     * @param newHiddenURI New hidden metadata URI
     */
    function setHiddenMetadataURI(string calldata newHiddenURI) 
        external 
        onlyRole(METADATA_ROLE) 
    {
        _hiddenMetadataURI = newHiddenURI;
    }
    
    /**
     * @dev Set contract metadata URI
     * @param newContractURI New contract metadata URI
     */
    function setContractURI(string calldata newContractURI) 
        external 
        onlyRole(METADATA_ROLE) 
    {
        _contractURI = newContractURI;
    }
    
    /**
     * @dev Reveal the collection
     */
    function reveal() external onlyRole(METADATA_ROLE) {
        revealed = true;
        emit RevealStatusChanged(true);
    }
    
    /**
     * @dev Set token URI for specific token
     * @param tokenId Token ID
     * @param uri Token URI
     */
    function setTokenURI(uint256 tokenId, string calldata uri) 
        external 
        onlyRole(METADATA_ROLE) 
    {
        require(_ownerOf(tokenId) != address(0), "ISANFT: token does not exist");
        _setTokenURI(tokenId, uri);
    }
    
    /**
     * @dev Update royalty information
     * @param recipient New royalty recipient
     * @param feeBps New royalty fee in basis points
     */
    function setRoyalty(address recipient, uint96 feeBps) 
        external 
        onlyRole(ROYALTY_ROLE) 
    {
        require(recipient != address(0), "ISANFT: recipient cannot be zero");
        require(feeBps <= 1000, "ISANFT: royalty fee too high"); // Max 10%
        
        royaltyRecipient = recipient;
        royaltyFeeBps = feeBps;
        _setDefaultRoyalty(recipient, feeBps);
        
        emit RoyaltyUpdated(recipient, feeBps);
    }
    
    /**
     * @dev Update withdraw split percentages
     * @param _treasuryShare Treasury share in basis points
     * @param _artistShare Artist share in basis points
     */
    function setWithdrawSplit(uint256 _treasuryShare, uint256 _artistShare) 
        external 
        onlyRole(DEFAULT_ADMIN_ROLE) 
    {
        require(_treasuryShare + _artistShare == 10000, "ISANFT: shares must equal 100%");
        
        treasuryShare = _treasuryShare;
        artistShare = _artistShare;
        
        emit WithdrawSplitUpdated(_treasuryShare, _artistShare);
    }
    
    /**
     * @dev Withdraw contract balance
     */
    function withdraw() external onlyRole(DEFAULT_ADMIN_ROLE) nonReentrant {
        uint256 balance = address(this).balance;
        require(balance > 0, "ISANFT: no funds to withdraw");
        
        uint256 treasuryAmount = (balance * treasuryShare) / 10000;
        uint256 artistAmount = balance - treasuryAmount;
        
        (bool treasurySuccess, ) = treasury.call{value: treasuryAmount}("");
        require(treasurySuccess, "ISANFT: treasury transfer failed");
        
        (bool artistSuccess, ) = artist.call{value: artistAmount}("");
        require(artistSuccess, "ISANFT: artist transfer failed");
    }
    
    /**
     * @dev Pause contract
     */
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }
    
    /**
     * @dev Unpause contract
     */
    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }
    
    /**
     * @dev Get token URI
     * @param tokenId Token ID
     * @return Token URI string
     */
    function tokenURI(uint256 tokenId) 
        public 
        view 
        override(ERC721, ERC721URIStorage) 
        returns (string memory) 
    {
        require(_ownerOf(tokenId) != address(0), "ISANFT: token does not exist");
        
        // Return custom URI if set
        string memory customURI = super.tokenURI(tokenId);
        if (bytes(customURI).length > 0) {
            return customURI;
        }
        
        // Return hidden metadata if not revealed
        if (!revealed) {
            return _hiddenMetadataURI;
        }
        
        // Return revealed metadata
        return string(abi.encodePacked(_baseTokenURI, _toString(tokenId), ".json"));
    }
    
    /**
     * @dev Get contract metadata URI
     * @return Contract metadata URI
     */
    function contractURI() external view returns (string memory) {
        return _contractURI;
    }
    
    /**
     * @dev Get total number of tokens minted
     * @return Current supply
     */
    function totalSupply() public view override returns (uint256) {
        return _currentIndex - 1;
    }
    
    /**
     * @dev Check if user is whitelisted
     * @param user Address to check
     * @return Boolean indicating whitelist status
     */
    function isWhitelisted(address user) external view returns (bool) {
        return whitelist[user];
    }
    
    /**
     * @dev Get collection information
     * @return Collection info struct
     */
    function getCollectionInfo() external view returns (
        uint256 currentSupply,
        uint256 maxSupply,
        uint256 publicPrice,
        uint256 whitelistPriceValue,
        bool publicEnabled,
        bool whitelistEnabled,
        bool isRevealed,
        uint256 whitelistTotal
    ) {
        return (
            totalSupply(),
            MAX_SUPPLY,
            mintPrice,
            whitelistPrice,
            publicMintEnabled,
            whitelistMintEnabled,
            revealed,
            whitelistCount
        );
    }
    
    /**
     * @dev Convert uint256 to string
     * @param value Number to convert
     * @return String representation
     */
    function _toString(uint256 value) internal pure returns (string memory) {
        if (value == 0) {
            return "0";
        }
        uint256 temp = value;
        uint256 digits;
        while (temp != 0) {
            digits++;
            temp /= 10;
        }
        bytes memory buffer = new bytes(digits);
        while (value != 0) {
            digits -= 1;
            buffer[digits] = bytes1(uint8(48 + uint256(value % 10)));
            value /= 10;
        }
        return string(buffer);
    }
    
    // Required overrides
    function _update(address to, uint256 tokenId, address auth)
        internal
        override(ERC721, ERC721Enumerable, ERC721Pausable)
        returns (address)
    {
        return super._update(to, tokenId, auth);
    }
    
    function _increaseBalance(address account, uint128 value)
        internal
        override(ERC721, ERC721Enumerable)
    {
        super._increaseBalance(account, value);
    }
    
    function supportsInterface(bytes4 interfaceId)
        public
        view
        override(ERC721, ERC721Enumerable, ERC721URIStorage, AccessControl, ERC2981)
        returns (bool)
    {
        return super.supportsInterface(interfaceId);
    }
}