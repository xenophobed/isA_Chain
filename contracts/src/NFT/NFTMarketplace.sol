// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC721/IERC721.sol";
import "@openzeppelin/contracts/token/ERC721/IERC721Receiver.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/token/ERC20/utils/SafeERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/security/ReentrancyGuard.sol";
import "@openzeppelin/contracts/security/Pausable.sol";
import "@openzeppelin/contracts/interfaces/IERC2981.sol";
import "@openzeppelin/contracts/utils/introspection/ERC165Checker.sol";

/**
 * @title NFTMarketplace
 * @dev Decentralized NFT marketplace for the isA_Chain ecosystem
 * 
 * Features:
 * - Buy/sell NFTs with ETH or ERC20 tokens
 * - Auction system with bidding
 * - Royalty support (ERC2981)
 * - Offers and counteroffers
 * - Collection-based trading
 * - Fee management
 * - Emergency functions
 */
contract NFTMarketplace is Ownable, ReentrancyGuard, Pausable, IERC721Receiver {
    using SafeERC20 for IERC20;
    using ERC165Checker for address;
    
    // Market fee configuration
    uint256 public platformFee = 250; // 2.5% in basis points
    uint256 public constant MAX_PLATFORM_FEE = 1000; // 10% max
    address public feeRecipient;
    
    // Supported payment tokens
    mapping(address => bool) public supportedTokens;
    address public constant ETH_ADDRESS = address(0);
    
    // Listing structure
    struct Listing {
        address seller;
        address nftContract;
        uint256 tokenId;
        address paymentToken;
        uint256 price;
        uint256 expiration;
        bool active;
    }
    
    // Auction structure
    struct Auction {
        address seller;
        address nftContract;
        uint256 tokenId;
        address paymentToken;
        uint256 startingPrice;
        uint256 currentBid;
        address currentBidder;
        uint256 endTime;
        uint256 minBidIncrement;
        bool active;
        bool settled;
    }
    
    // Offer structure
    struct Offer {
        address buyer;
        address nftContract;
        uint256 tokenId;
        address paymentToken;
        uint256 amount;
        uint256 expiration;
        bool active;
    }
    
    // Storage mappings
    mapping(bytes32 => Listing) public listings;
    mapping(bytes32 => Auction) public auctions;
    mapping(bytes32 => Offer) public offers;
    
    // User-specific mappings
    mapping(address => bytes32[]) public userListings;
    mapping(address => bytes32[]) public userAuctions;
    mapping(address => bytes32[]) public userOffers;
    
    // Collection settings
    mapping(address => bool) public approvedCollections;
    mapping(address => uint256) public collectionFees; // Custom fees per collection
    
    // Bidding tracking
    mapping(bytes32 => address[]) public auctionBidders;
    mapping(bytes32 => mapping(address => uint256)) public pendingReturns;
    
    // Events
    event NFTListed(
        bytes32 indexed listingId,
        address indexed seller,
        address indexed nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 price,
        uint256 expiration
    );
    
    event NFTSold(
        bytes32 indexed listingId,
        address indexed buyer,
        address indexed seller,
        address nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 price,
        uint256 platformFeeAmount,
        uint256 royaltyAmount
    );
    
    event AuctionCreated(
        bytes32 indexed auctionId,
        address indexed seller,
        address indexed nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 startingPrice,
        uint256 endTime
    );
    
    event BidPlaced(
        bytes32 indexed auctionId,
        address indexed bidder,
        uint256 amount
    );
    
    event AuctionSettled(
        bytes32 indexed auctionId,
        address indexed winner,
        uint256 winningBid
    );
    
    event OfferMade(
        bytes32 indexed offerId,
        address indexed buyer,
        address indexed nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 amount,
        uint256 expiration
    );
    
    event OfferAccepted(
        bytes32 indexed offerId,
        address indexed seller,
        address indexed buyer,
        uint256 amount
    );
    
    event ListingCancelled(bytes32 indexed listingId);
    event AuctionCancelled(bytes32 indexed auctionId);
    event OfferCancelled(bytes32 indexed offerId);
    
    event PlatformFeeUpdated(uint256 newFee);
    event FeeRecipientUpdated(address newRecipient);
    event TokenSupportUpdated(address token, bool supported);
    event CollectionApproved(address collection, bool approved);
    
    /**
     * @dev Constructor
     * @param _feeRecipient Address to receive platform fees
     */
    constructor(address _feeRecipient) {
        require(_feeRecipient != address(0), "NFTMarketplace: invalid fee recipient");
        feeRecipient = _feeRecipient;
        _transferOwnership(msg.sender);
        
        // ETH is supported by default
        supportedTokens[ETH_ADDRESS] = true;
    }
    
    /**
     * @dev List an NFT for sale
     * @param nftContract NFT contract address
     * @param tokenId Token ID to list
     * @param paymentToken Payment token address (0 for ETH)
     * @param price Listing price
     * @param expiration Listing expiration timestamp
     */
    function listNFT(
        address nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 price,
        uint256 expiration
    ) external whenNotPaused nonReentrant returns (bytes32) {
        require(approvedCollections[nftContract], "NFTMarketplace: collection not approved");
        require(supportedTokens[paymentToken], "NFTMarketplace: payment token not supported");
        require(price > 0, "NFTMarketplace: price must be positive");
        require(expiration > block.timestamp, "NFTMarketplace: invalid expiration");
        
        IERC721 nft = IERC721(nftContract);
        require(nft.ownerOf(tokenId) == msg.sender, "NFTMarketplace: not token owner");
        require(nft.isApprovedForAll(msg.sender, address(this)) || 
                nft.getApproved(tokenId) == address(this), 
                "NFTMarketplace: marketplace not approved");
        
        bytes32 listingId = keccak256(abi.encodePacked(
            msg.sender,
            nftContract,
            tokenId,
            block.timestamp,
            block.number
        ));
        
        listings[listingId] = Listing({
            seller: msg.sender,
            nftContract: nftContract,
            tokenId: tokenId,
            paymentToken: paymentToken,
            price: price,
            expiration: expiration,
            active: true
        });
        
        userListings[msg.sender].push(listingId);
        
        emit NFTListed(listingId, msg.sender, nftContract, tokenId, paymentToken, price, expiration);
        return listingId;
    }
    
    /**
     * @dev Buy a listed NFT
     * @param listingId Listing identifier
     */
    function buyNFT(bytes32 listingId) external payable whenNotPaused nonReentrant {
        Listing storage listing = listings[listingId];
        require(listing.active, "NFTMarketplace: listing not active");
        require(block.timestamp <= listing.expiration, "NFTMarketplace: listing expired");
        require(msg.sender != listing.seller, "NFTMarketplace: cannot buy own NFT");
        
        listing.active = false;
        
        // Handle payment
        uint256 totalAmount = listing.price;
        (uint256 platformFeeAmount, uint256 royaltyAmount, address royaltyRecipient) = 
            _calculateFees(listing.nftContract, listing.tokenId, totalAmount);
        
        uint256 sellerAmount = totalAmount - platformFeeAmount - royaltyAmount;
        
        if (listing.paymentToken == ETH_ADDRESS) {
            require(msg.value >= totalAmount, "NFTMarketplace: insufficient ETH");
            
            // Transfer payments
            if (platformFeeAmount > 0) {
                (bool feeSuccess, ) = feeRecipient.call{value: platformFeeAmount}("");
                require(feeSuccess, "NFTMarketplace: fee transfer failed");
            }
            
            if (royaltyAmount > 0 && royaltyRecipient != address(0)) {
                (bool royaltySuccess, ) = royaltyRecipient.call{value: royaltyAmount}("");
                require(royaltySuccess, "NFTMarketplace: royalty transfer failed");
            }
            
            (bool sellerSuccess, ) = listing.seller.call{value: sellerAmount}("");
            require(sellerSuccess, "NFTMarketplace: seller transfer failed");
            
            // Refund excess ETH
            if (msg.value > totalAmount) {
                (bool refundSuccess, ) = msg.sender.call{value: msg.value - totalAmount}("");
                require(refundSuccess, "NFTMarketplace: refund failed");
            }
        } else {
            IERC20 token = IERC20(listing.paymentToken);
            
            // Transfer from buyer
            token.safeTransferFrom(msg.sender, address(this), totalAmount);
            
            // Distribute payments
            if (platformFeeAmount > 0) {
                token.safeTransfer(feeRecipient, platformFeeAmount);
            }
            
            if (royaltyAmount > 0 && royaltyRecipient != address(0)) {
                token.safeTransfer(royaltyRecipient, royaltyAmount);
            }
            
            token.safeTransfer(listing.seller, sellerAmount);
        }
        
        // Transfer NFT
        IERC721(listing.nftContract).safeTransferFrom(
            listing.seller,
            msg.sender,
            listing.tokenId
        );
        
        emit NFTSold(
            listingId,
            msg.sender,
            listing.seller,
            listing.nftContract,
            listing.tokenId,
            listing.paymentToken,
            totalAmount,
            platformFeeAmount,
            royaltyAmount
        );
    }
    
    /**
     * @dev Create an auction for an NFT
     * @param nftContract NFT contract address
     * @param tokenId Token ID to auction
     * @param paymentToken Payment token address
     * @param startingPrice Starting bid price
     * @param duration Auction duration in seconds
     * @param minBidIncrement Minimum bid increment
     */
    function createAuction(
        address nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 startingPrice,
        uint256 duration,
        uint256 minBidIncrement
    ) external whenNotPaused nonReentrant returns (bytes32) {
        require(approvedCollections[nftContract], "NFTMarketplace: collection not approved");
        require(supportedTokens[paymentToken], "NFTMarketplace: payment token not supported");
        require(startingPrice > 0, "NFTMarketplace: starting price must be positive");
        require(duration >= 1 hours && duration <= 30 days, "NFTMarketplace: invalid duration");
        require(minBidIncrement > 0, "NFTMarketplace: min bid increment must be positive");
        
        IERC721 nft = IERC721(nftContract);
        require(nft.ownerOf(tokenId) == msg.sender, "NFTMarketplace: not token owner");
        require(nft.isApprovedForAll(msg.sender, address(this)) || 
                nft.getApproved(tokenId) == address(this), 
                "NFTMarketplace: marketplace not approved");
        
        bytes32 auctionId = keccak256(abi.encodePacked(
            msg.sender,
            nftContract,
            tokenId,
            block.timestamp,
            block.number
        ));
        
        uint256 endTime = block.timestamp + duration;
        
        auctions[auctionId] = Auction({
            seller: msg.sender,
            nftContract: nftContract,
            tokenId: tokenId,
            paymentToken: paymentToken,
            startingPrice: startingPrice,
            currentBid: 0,
            currentBidder: address(0),
            endTime: endTime,
            minBidIncrement: minBidIncrement,
            active: true,
            settled: false
        });
        
        userAuctions[msg.sender].push(auctionId);
        
        // Transfer NFT to marketplace for escrow
        nft.safeTransferFrom(msg.sender, address(this), tokenId);
        
        emit AuctionCreated(auctionId, msg.sender, nftContract, tokenId, paymentToken, startingPrice, endTime);
        return auctionId;
    }
    
    /**
     * @dev Place a bid on an auction
     * @param auctionId Auction identifier
     * @param bidAmount Bid amount (for ERC20 tokens)
     */
    function placeBid(bytes32 auctionId, uint256 bidAmount) external payable whenNotPaused nonReentrant {
        Auction storage auction = auctions[auctionId];
        require(auction.active, "NFTMarketplace: auction not active");
        require(block.timestamp < auction.endTime, "NFTMarketplace: auction ended");
        require(msg.sender != auction.seller, "NFTMarketplace: seller cannot bid");
        
        uint256 bid;
        if (auction.paymentToken == ETH_ADDRESS) {
            bid = msg.value;
        } else {
            bid = bidAmount;
            require(msg.value == 0, "NFTMarketplace: ETH not accepted for this auction");
        }
        
        uint256 minBid = auction.currentBid > 0 ? 
            auction.currentBid + auction.minBidIncrement : 
            auction.startingPrice;
        
        require(bid >= minBid, "NFTMarketplace: bid too low");
        
        // Refund previous bidder
        if (auction.currentBidder != address(0)) {
            if (auction.paymentToken == ETH_ADDRESS) {
                pendingReturns[auctionId][auction.currentBidder] += auction.currentBid;
            } else {
                IERC20(auction.paymentToken).safeTransfer(auction.currentBidder, auction.currentBid);
            }
        }
        
        // Handle new bid
        if (auction.paymentToken != ETH_ADDRESS) {
            IERC20(auction.paymentToken).safeTransferFrom(msg.sender, address(this), bid);
        }
        
        auction.currentBid = bid;
        auction.currentBidder = msg.sender;
        
        // Track bidder for potential refunds
        if (pendingReturns[auctionId][msg.sender] == 0) {
            auctionBidders[auctionId].push(msg.sender);
        }
        
        // Extend auction if bid placed in last 10 minutes
        if (auction.endTime - block.timestamp < 10 minutes) {
            auction.endTime = block.timestamp + 10 minutes;
        }
        
        emit BidPlaced(auctionId, msg.sender, bid);
    }
    
    /**
     * @dev Settle an auction
     * @param auctionId Auction identifier
     */
    function settleAuction(bytes32 auctionId) external nonReentrant {
        Auction storage auction = auctions[auctionId];
        require(auction.active, "NFTMarketplace: auction not active");
        require(block.timestamp >= auction.endTime, "NFTMarketplace: auction still active");
        require(!auction.settled, "NFTMarketplace: auction already settled");
        
        auction.active = false;
        auction.settled = true;
        
        if (auction.currentBidder != address(0)) {
            // Calculate fees
            (uint256 platformFeeAmount, uint256 royaltyAmount, address royaltyRecipient) = 
                _calculateFees(auction.nftContract, auction.tokenId, auction.currentBid);
            
            uint256 sellerAmount = auction.currentBid - platformFeeAmount - royaltyAmount;
            
            // Transfer payments
            if (auction.paymentToken == ETH_ADDRESS) {
                if (platformFeeAmount > 0) {
                    (bool feeSuccess, ) = feeRecipient.call{value: platformFeeAmount}("");
                    require(feeSuccess, "NFTMarketplace: fee transfer failed");
                }
                
                if (royaltyAmount > 0 && royaltyRecipient != address(0)) {
                    (bool royaltySuccess, ) = royaltyRecipient.call{value: royaltyAmount}("");
                    require(royaltySuccess, "NFTMarketplace: royalty transfer failed");
                }
                
                (bool sellerSuccess, ) = auction.seller.call{value: sellerAmount}("");
                require(sellerSuccess, "NFTMarketplace: seller transfer failed");
            } else {
                IERC20 token = IERC20(auction.paymentToken);
                
                if (platformFeeAmount > 0) {
                    token.safeTransfer(feeRecipient, platformFeeAmount);
                }
                
                if (royaltyAmount > 0 && royaltyRecipient != address(0)) {
                    token.safeTransfer(royaltyRecipient, royaltyAmount);
                }
                
                token.safeTransfer(auction.seller, sellerAmount);
            }
            
            // Transfer NFT to winner
            IERC721(auction.nftContract).safeTransferFrom(
                address(this),
                auction.currentBidder,
                auction.tokenId
            );
            
            emit AuctionSettled(auctionId, auction.currentBidder, auction.currentBid);
        } else {
            // No bids, return NFT to seller
            IERC721(auction.nftContract).safeTransferFrom(
                address(this),
                auction.seller,
                auction.tokenId
            );
        }
    }
    
    /**
     * @dev Make an offer on an NFT
     * @param nftContract NFT contract address
     * @param tokenId Token ID
     * @param paymentToken Payment token address
     * @param amount Offer amount
     * @param expiration Offer expiration timestamp
     */
    function makeOffer(
        address nftContract,
        uint256 tokenId,
        address paymentToken,
        uint256 amount,
        uint256 expiration
    ) external payable whenNotPaused nonReentrant returns (bytes32) {
        require(approvedCollections[nftContract], "NFTMarketplace: collection not approved");
        require(supportedTokens[paymentToken], "NFTMarketplace: payment token not supported");
        require(amount > 0, "NFTMarketplace: amount must be positive");
        require(expiration > block.timestamp, "NFTMarketplace: invalid expiration");
        
        bytes32 offerId = keccak256(abi.encodePacked(
            msg.sender,
            nftContract,
            tokenId,
            amount,
            block.timestamp,
            block.number
        ));
        
        // Handle payment escrow
        if (paymentToken == ETH_ADDRESS) {
            require(msg.value >= amount, "NFTMarketplace: insufficient ETH");
            if (msg.value > amount) {
                (bool refundSuccess, ) = msg.sender.call{value: msg.value - amount}("");
                require(refundSuccess, "NFTMarketplace: refund failed");
            }
        } else {
            require(msg.value == 0, "NFTMarketplace: ETH not accepted");
            IERC20(paymentToken).safeTransferFrom(msg.sender, address(this), amount);
        }
        
        offers[offerId] = Offer({
            buyer: msg.sender,
            nftContract: nftContract,
            tokenId: tokenId,
            paymentToken: paymentToken,
            amount: amount,
            expiration: expiration,
            active: true
        });
        
        userOffers[msg.sender].push(offerId);
        
        emit OfferMade(offerId, msg.sender, nftContract, tokenId, paymentToken, amount, expiration);
        return offerId;
    }
    
    /**
     * @dev Accept an offer
     * @param offerId Offer identifier
     */
    function acceptOffer(bytes32 offerId) external whenNotPaused nonReentrant {
        Offer storage offer = offers[offerId];
        require(offer.active, "NFTMarketplace: offer not active");
        require(block.timestamp <= offer.expiration, "NFTMarketplace: offer expired");
        
        IERC721 nft = IERC721(offer.nftContract);
        require(nft.ownerOf(offer.tokenId) == msg.sender, "NFTMarketplace: not token owner");
        require(nft.isApprovedForAll(msg.sender, address(this)) || 
                nft.getApproved(offer.tokenId) == address(this), 
                "NFTMarketplace: marketplace not approved");
        
        offer.active = false;
        
        // Calculate fees
        (uint256 platformFeeAmount, uint256 royaltyAmount, address royaltyRecipient) = 
            _calculateFees(offer.nftContract, offer.tokenId, offer.amount);
        
        uint256 sellerAmount = offer.amount - platformFeeAmount - royaltyAmount;
        
        // Transfer payments
        if (offer.paymentToken == ETH_ADDRESS) {
            if (platformFeeAmount > 0) {
                (bool feeSuccess, ) = feeRecipient.call{value: platformFeeAmount}("");
                require(feeSuccess, "NFTMarketplace: fee transfer failed");
            }
            
            if (royaltyAmount > 0 && royaltyRecipient != address(0)) {
                (bool royaltySuccess, ) = royaltyRecipient.call{value: royaltyAmount}("");
                require(royaltySuccess, "NFTMarketplace: royalty transfer failed");
            }
            
            (bool sellerSuccess, ) = msg.sender.call{value: sellerAmount}("");
            require(sellerSuccess, "NFTMarketplace: seller transfer failed");
        } else {
            IERC20 token = IERC20(offer.paymentToken);
            
            if (platformFeeAmount > 0) {
                token.safeTransfer(feeRecipient, platformFeeAmount);
            }
            
            if (royaltyAmount > 0 && royaltyRecipient != address(0) && royaltyRecipient != msg.sender) {
                token.safeTransfer(royaltyRecipient, royaltyAmount);
            } else if (royaltyAmount > 0) {
                sellerAmount += royaltyAmount; // Add back if seller is royalty recipient
            }
            
            token.safeTransfer(msg.sender, sellerAmount);
        }
        
        // Transfer NFT
        nft.safeTransferFrom(msg.sender, offer.buyer, offer.tokenId);
        
        emit OfferAccepted(offerId, msg.sender, offer.buyer, offer.amount);
    }
    
    /**
     * @dev Cancel a listing
     * @param listingId Listing identifier
     */
    function cancelListing(bytes32 listingId) external nonReentrant {
        Listing storage listing = listings[listingId];
        require(listing.active, "NFTMarketplace: listing not active");
        require(msg.sender == listing.seller || msg.sender == owner(), "NFTMarketplace: unauthorized");
        
        listing.active = false;
        emit ListingCancelled(listingId);
    }
    
    /**
     * @dev Cancel an auction (only before first bid)
     * @param auctionId Auction identifier
     */
    function cancelAuction(bytes32 auctionId) external nonReentrant {
        Auction storage auction = auctions[auctionId];
        require(auction.active, "NFTMarketplace: auction not active");
        require(auction.currentBid == 0, "NFTMarketplace: auction has bids");
        require(msg.sender == auction.seller || msg.sender == owner(), "NFTMarketplace: unauthorized");
        
        auction.active = false;
        
        // Return NFT to seller
        IERC721(auction.nftContract).safeTransferFrom(
            address(this),
            auction.seller,
            auction.tokenId
        );
        
        emit AuctionCancelled(auctionId);
    }
    
    /**
     * @dev Cancel an offer
     * @param offerId Offer identifier
     */
    function cancelOffer(bytes32 offerId) external nonReentrant {
        Offer storage offer = offers[offerId];
        require(offer.active, "NFTMarketplace: offer not active");
        require(msg.sender == offer.buyer || msg.sender == owner(), "NFTMarketplace: unauthorized");
        
        offer.active = false;
        
        // Refund escrowed payment
        if (offer.paymentToken == ETH_ADDRESS) {
            (bool success, ) = offer.buyer.call{value: offer.amount}("");
            require(success, "NFTMarketplace: refund failed");
        } else {
            IERC20(offer.paymentToken).safeTransfer(offer.buyer, offer.amount);
        }
        
        emit OfferCancelled(offerId);
    }
    
    /**
     * @dev Withdraw pending returns for failed auction bids
     * @param auctionId Auction identifier
     */
    function withdrawPendingReturns(bytes32 auctionId) external nonReentrant {
        uint256 amount = pendingReturns[auctionId][msg.sender];
        require(amount > 0, "NFTMarketplace: no pending returns");
        
        pendingReturns[auctionId][msg.sender] = 0;
        
        (bool success, ) = msg.sender.call{value: amount}("");
        require(success, "NFTMarketplace: withdrawal failed");
    }
    
    /**
     * @dev Calculate platform and royalty fees
     * @param nftContract NFT contract address
     * @param tokenId Token ID
     * @param salePrice Sale price
     * @return platformFeeAmount Platform fee amount
     * @return royaltyAmount Royalty amount
     * @return royaltyRecipient Royalty recipient
     */
    function _calculateFees(address nftContract, uint256 tokenId, uint256 salePrice) 
        internal 
        view 
        returns (uint256 platformFeeAmount, uint256 royaltyAmount, address royaltyRecipient) 
    {
        // Calculate platform fee
        uint256 fee = collectionFees[nftContract] > 0 ? collectionFees[nftContract] : platformFee;
        platformFeeAmount = (salePrice * fee) / 10000;
        
        // Calculate royalty
        if (nftContract.supportsInterface(type(IERC2981).interfaceId)) {
            (royaltyRecipient, royaltyAmount) = IERC2981(nftContract).royaltyInfo(tokenId, salePrice);
        }
        
        return (platformFeeAmount, royaltyAmount, royaltyRecipient);
    }
    
    // Admin functions
    function setPlatformFee(uint256 newFee) external onlyOwner {
        require(newFee <= MAX_PLATFORM_FEE, "NFTMarketplace: fee too high");
        platformFee = newFee;
        emit PlatformFeeUpdated(newFee);
    }
    
    function setFeeRecipient(address newRecipient) external onlyOwner {
        require(newRecipient != address(0), "NFTMarketplace: invalid recipient");
        feeRecipient = newRecipient;
        emit FeeRecipientUpdated(newRecipient);
    }
    
    function setSupportedToken(address token, bool supported) external onlyOwner {
        supportedTokens[token] = supported;
        emit TokenSupportUpdated(token, supported);
    }
    
    function setCollectionApproval(address collection, bool approved) external onlyOwner {
        approvedCollections[collection] = approved;
        emit CollectionApproved(collection, approved);
    }
    
    function setCollectionFee(address collection, uint256 fee) external onlyOwner {
        require(fee <= MAX_PLATFORM_FEE, "NFTMarketplace: fee too high");
        collectionFees[collection] = fee;
    }
    
    function pause() external onlyOwner {
        _pause();
    }
    
    function unpause() external onlyOwner {
        _unpause();
    }
    
    // Required for receiving NFTs
    function onERC721Received(address, address, uint256, bytes calldata) 
        external 
        pure 
        override 
        returns (bytes4) 
    {
        return IERC721Receiver.onERC721Received.selector;
    }
    
    // Emergency function to recover stuck NFTs (only owner)
    function emergencyRecoverNFT(address nftContract, uint256 tokenId, address to) 
        external 
        onlyOwner 
    {
        IERC721(nftContract).safeTransferFrom(address(this), to, tokenId);
    }
    
    // Emergency function to recover stuck tokens (only owner)
    function emergencyRecoverToken(address token, uint256 amount, address to) 
        external 
        onlyOwner 
    {
        if (token == ETH_ADDRESS) {
            (bool success, ) = to.call{value: amount}("");
            require(success, "NFTMarketplace: ETH recovery failed");
        } else {
            IERC20(token).safeTransfer(to, amount);
        }
    }
}