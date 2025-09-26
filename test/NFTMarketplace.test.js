const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("NFTMarketplace", function () {
  let marketplace, nft, token;
  let owner, feeRecipient, seller, buyer, artist, treasury, royaltyRecipient;
  let listingId, auctionId, offerId;
  
  beforeEach(async function () {
    [owner, feeRecipient, seller, buyer, artist, treasury, royaltyRecipient] = await ethers.getSigners();
    
    // Deploy NFT contract for testing
    const ISANFT = await ethers.getContractFactory("ISANFT");
    nft = await ISANFT.deploy(treasury.address, artist.address, royaltyRecipient.address);
    await nft.deployed();
    
    // Deploy test ERC20 token
    const SimpleToken = await ethers.getContractFactory("SimpleToken");
    token = await SimpleToken.deploy();
    await token.deployed();
    
    // Deploy marketplace
    const NFTMarketplace = await ethers.getContractFactory("NFTMarketplace");
    marketplace = await NFTMarketplace.deploy(feeRecipient.address);
    await marketplace.deployed();
    
    // Setup: approve collection and mint test NFTs
    await marketplace.setCollectionApproval(nft.address, true);
    await nft.adminMint(seller.address, 5); // Mint tokens 1-5 to seller
    
    // Approve marketplace to transfer NFTs
    await nft.connect(seller).setApprovalForAll(marketplace.address, true);
    
    // Give buyer some test tokens
    await token.transfer(buyer.address, ethers.utils.parseEther("1000"));
    await token.connect(buyer).approve(marketplace.address, ethers.utils.parseEther("1000"));
  });

  describe("Deployment", function () {
    it("Should set correct fee recipient", async function () {
      expect(await marketplace.feeRecipient()).to.equal(feeRecipient.address);
    });

    it("Should set correct platform fee", async function () {
      expect(await marketplace.platformFee()).to.equal(250); // 2.5%
    });

    it("Should support ETH by default", async function () {
      const ETH_ADDRESS = await marketplace.ETH_ADDRESS();
      expect(await marketplace.supportedTokens(ETH_ADDRESS)).to.be.true;
    });

    it("Should set correct owner", async function () {
      expect(await marketplace.owner()).to.equal(owner.address);
    });
  });

  describe("Collection Management", function () {
    it("Should approve collection", async function () {
      const newCollection = await (await ethers.getContractFactory("ISANFT"))
        .deploy(treasury.address, artist.address, royaltyRecipient.address);
      
      await expect(marketplace.setCollectionApproval(newCollection.address, true))
        .to.emit(marketplace, "CollectionApproved")
        .withArgs(newCollection.address, true);
        
      expect(await marketplace.approvedCollections(newCollection.address)).to.be.true;
    });

    it("Should disapprove collection", async function () {
      await marketplace.setCollectionApproval(nft.address, false);
      expect(await marketplace.approvedCollections(nft.address)).to.be.false;
    });

    it("Should not allow non-owner to manage collections", async function () {
      await expect(
        marketplace.connect(seller).setCollectionApproval(nft.address, true)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("Payment Token Management", function () {
    it("Should add supported token", async function () {
      await expect(marketplace.setSupportedToken(token.address, true))
        .to.emit(marketplace, "TokenSupportUpdated")
        .withArgs(token.address, true);
        
      expect(await marketplace.supportedTokens(token.address)).to.be.true;
    });

    it("Should remove supported token", async function () {
      await marketplace.setSupportedToken(token.address, true);
      await marketplace.setSupportedToken(token.address, false);
      
      expect(await marketplace.supportedTokens(token.address)).to.be.false;
    });

    it("Should not allow non-owner to manage tokens", async function () {
      await expect(
        marketplace.connect(seller).setSupportedToken(token.address, true)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("NFT Listing", function () {
    it("Should list NFT for sale", async function () {
      const price = ethers.utils.parseEther("1");
      const expiration = Math.floor(Date.now() / 1000) + 3600; // 1 hour from now
      const ETH_ADDRESS = await marketplace.ETH_ADDRESS();
      
      await expect(
        marketplace.connect(seller).listNFT(
          nft.address,
          1,
          ETH_ADDRESS,
          price,
          expiration
        )
      ).to.emit(marketplace, "NFTListed");
      
      // Get the listing ID from events
      const filter = marketplace.filters.NFTListed();
      const events = await marketplace.queryFilter(filter);
      listingId = events[events.length - 1].args.listingId;
      
      const listing = await marketplace.listings(listingId);
      expect(listing.seller).to.equal(seller.address);
      expect(listing.nftContract).to.equal(nft.address);
      expect(listing.tokenId).to.equal(1);
      expect(listing.price).to.equal(price);
      expect(listing.active).to.be.true;
    });

    it("Should not list NFT from non-approved collection", async function () {
      await marketplace.setCollectionApproval(nft.address, false);
      
      await expect(
        marketplace.connect(seller).listNFT(
          nft.address,
          1,
          await marketplace.ETH_ADDRESS(),
          ethers.utils.parseEther("1"),
          Math.floor(Date.now() / 1000) + 3600
        )
      ).to.be.revertedWith("NFTMarketplace: collection not approved");
    });

    it("Should not list NFT with unsupported payment token", async function () {
      await expect(
        marketplace.connect(seller).listNFT(
          nft.address,
          1,
          token.address, // Not supported yet
          ethers.utils.parseEther("1"),
          Math.floor(Date.now() / 1000) + 3600
        )
      ).to.be.revertedWith("NFTMarketplace: payment token not supported");
    });

    it("Should not list NFT if not owner", async function () {
      await expect(
        marketplace.connect(buyer).listNFT(
          nft.address,
          1,
          await marketplace.ETH_ADDRESS(),
          ethers.utils.parseEther("1"),
          Math.floor(Date.now() / 1000) + 3600
        )
      ).to.be.revertedWith("NFTMarketplace: not token owner");
    });

    it("Should not list with zero price", async function () {
      await expect(
        marketplace.connect(seller).listNFT(
          nft.address,
          1,
          await marketplace.ETH_ADDRESS(),
          0,
          Math.floor(Date.now() / 1000) + 3600
        )
      ).to.be.revertedWith("NFTMarketplace: price must be positive");
    });
  });

  describe("NFT Purchase with ETH", function () {
    beforeEach(async function () {
      // List NFT for sale
      const price = ethers.utils.parseEther("1");
      const expiration = Math.floor(Date.now() / 1000) + 3600;
      const ETH_ADDRESS = await marketplace.ETH_ADDRESS();
      
      const tx = await marketplace.connect(seller).listNFT(
        nft.address,
        1,
        ETH_ADDRESS,
        price,
        expiration
      );
      
      const receipt = await tx.wait();
      listingId = receipt.events.find(e => e.event === "NFTListed").args.listingId;
    });

    it("Should buy NFT with ETH", async function () {
      const listing = await marketplace.listings(listingId);
      const price = listing.price;
      
      // Calculate expected fees
      const platformFeeAmount = price.mul(250).div(10000); // 2.5%
      const royaltyAmount = price.mul(750).div(10000); // 7.5% (from NFT contract)
      const sellerAmount = price.sub(platformFeeAmount).sub(royaltyAmount);
      
      const sellerBalanceBefore = await ethers.provider.getBalance(seller.address);
      const feeRecipientBalanceBefore = await ethers.provider.getBalance(feeRecipient.address);
      const royaltyRecipientBalanceBefore = await ethers.provider.getBalance(royaltyRecipient.address);
      
      await expect(
        marketplace.connect(buyer).buyNFT(listingId, { value: price })
      ).to.emit(marketplace, "NFTSold");
      
      // Check NFT ownership transferred
      expect(await nft.ownerOf(1)).to.equal(buyer.address);
      
      // Check payments distributed correctly
      const sellerBalanceAfter = await ethers.provider.getBalance(seller.address);
      const feeRecipientBalanceAfter = await ethers.provider.getBalance(feeRecipient.address);
      const royaltyRecipientBalanceAfter = await ethers.provider.getBalance(royaltyRecipient.address);
      
      expect(sellerBalanceAfter.sub(sellerBalanceBefore)).to.equal(sellerAmount);
      expect(feeRecipientBalanceAfter.sub(feeRecipientBalanceBefore)).to.equal(platformFeeAmount);
      expect(royaltyRecipientBalanceAfter.sub(royaltyRecipientBalanceBefore)).to.equal(royaltyAmount);
      
      // Check listing is no longer active
      const updatedListing = await marketplace.listings(listingId);
      expect(updatedListing.active).to.be.false;
    });

    it("Should refund excess ETH", async function () {
      const listing = await marketplace.listings(listingId);
      const price = listing.price;
      const overpayment = ethers.utils.parseEther("0.5");
      
      const buyerBalanceBefore = await ethers.provider.getBalance(buyer.address);
      
      const tx = await marketplace.connect(buyer).buyNFT(listingId, { 
        value: price.add(overpayment) 
      });
      const receipt = await tx.wait();
      const gasUsed = receipt.gasUsed.mul(receipt.effectiveGasPrice);
      
      const buyerBalanceAfter = await ethers.provider.getBalance(buyer.address);
      
      // Should only pay the listing price plus gas
      expect(buyerBalanceBefore.sub(buyerBalanceAfter).sub(gasUsed)).to.equal(price);
    });

    it("Should not buy with insufficient ETH", async function () {
      const listing = await marketplace.listings(listingId);
      const insufficientAmount = listing.price.sub(1);
      
      await expect(
        marketplace.connect(buyer).buyNFT(listingId, { value: insufficientAmount })
      ).to.be.revertedWith("NFTMarketplace: insufficient ETH");
    });

    it("Should not buy own NFT", async function () {
      const listing = await marketplace.listings(listingId);
      
      await expect(
        marketplace.connect(seller).buyNFT(listingId, { value: listing.price })
      ).to.be.revertedWith("NFTMarketplace: cannot buy own NFT");
    });
  });

  describe("NFT Purchase with ERC20", function () {
    beforeEach(async function () {
      // Add token support
      await marketplace.setSupportedToken(token.address, true);
      
      // List NFT for sale with ERC20
      const price = ethers.utils.parseEther("100");
      const expiration = Math.floor(Date.now() / 1000) + 3600;
      
      const tx = await marketplace.connect(seller).listNFT(
        nft.address,
        2,
        token.address,
        price,
        expiration
      );
      
      const receipt = await tx.wait();
      listingId = receipt.events.find(e => e.event === "NFTListed").args.listingId;
    });

    it("Should buy NFT with ERC20 token", async function () {
      const listing = await marketplace.listings(listingId);
      const price = listing.price;
      
      const sellerBalanceBefore = await token.balanceOf(seller.address);
      const buyerBalanceBefore = await token.balanceOf(buyer.address);
      const feeRecipientBalanceBefore = await token.balanceOf(feeRecipient.address);
      
      await marketplace.connect(buyer).buyNFT(listingId);
      
      // Check NFT ownership transferred
      expect(await nft.ownerOf(2)).to.equal(buyer.address);
      
      // Check token balances
      const sellerBalanceAfter = await token.balanceOf(seller.address);
      const buyerBalanceAfter = await token.balanceOf(buyer.address);
      const feeRecipientBalanceAfter = await token.balanceOf(feeRecipient.address);
      
      const platformFeeAmount = price.mul(250).div(10000);
      const royaltyAmount = price.mul(750).div(10000);
      const sellerAmount = price.sub(platformFeeAmount).sub(royaltyAmount);
      
      expect(sellerBalanceAfter.sub(sellerBalanceBefore)).to.equal(sellerAmount);
      expect(buyerBalanceBefore.sub(buyerBalanceAfter)).to.equal(price);
      expect(feeRecipientBalanceAfter.sub(feeRecipientBalanceBefore)).to.equal(platformFeeAmount);
    });
  });

  describe("Listing Cancellation", function () {
    beforeEach(async function () {
      const price = ethers.utils.parseEther("1");
      const expiration = Math.floor(Date.now() / 1000) + 3600;
      const ETH_ADDRESS = await marketplace.ETH_ADDRESS();
      
      const tx = await marketplace.connect(seller).listNFT(
        nft.address,
        3,
        ETH_ADDRESS,
        price,
        expiration
      );
      
      const receipt = await tx.wait();
      listingId = receipt.events.find(e => e.event === "NFTListed").args.listingId;
    });

    it("Should allow seller to cancel listing", async function () {
      await expect(marketplace.connect(seller).cancelListing(listingId))
        .to.emit(marketplace, "ListingCancelled")
        .withArgs(listingId);
        
      const listing = await marketplace.listings(listingId);
      expect(listing.active).to.be.false;
    });

    it("Should not allow non-seller to cancel listing", async function () {
      await expect(
        marketplace.connect(buyer).cancelListing(listingId)
      ).to.be.revertedWith("NFTMarketplace: unauthorized");
    });

    it("Should not cancel already inactive listing", async function () {
      await marketplace.connect(seller).cancelListing(listingId);
      
      await expect(
        marketplace.connect(seller).cancelListing(listingId)
      ).to.be.revertedWith("NFTMarketplace: listing not active");
    });
  });

  describe("Platform Fee Management", function () {
    it("Should update platform fee", async function () {
      const newFee = 500; // 5%
      
      await expect(marketplace.setPlatformFee(newFee))
        .to.emit(marketplace, "PlatformFeeUpdated")
        .withArgs(newFee);
        
      expect(await marketplace.platformFee()).to.equal(newFee);
    });

    it("Should not allow fee over maximum", async function () {
      const maxFee = await marketplace.MAX_PLATFORM_FEE();
      
      await expect(
        marketplace.setPlatformFee(maxFee.add(1))
      ).to.be.revertedWith("NFTMarketplace: fee too high");
    });

    it("Should not allow non-owner to update fee", async function () {
      await expect(
        marketplace.connect(seller).setPlatformFee(500)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });

    it("Should update fee recipient", async function () {
      await expect(marketplace.setFeeRecipient(buyer.address))
        .to.emit(marketplace, "FeeRecipientUpdated")
        .withArgs(buyer.address);
        
      expect(await marketplace.feeRecipient()).to.equal(buyer.address);
    });
  });

  describe("Pausing", function () {
    it("Should allow owner to pause", async function () {
      await marketplace.pause();
      expect(await marketplace.paused()).to.be.true;
    });

    it("Should not allow listing when paused", async function () {
      await marketplace.pause();
      
      await expect(
        marketplace.connect(seller).listNFT(
          nft.address,
          4,
          await marketplace.ETH_ADDRESS(),
          ethers.utils.parseEther("1"),
          Math.floor(Date.now() / 1000) + 3600
        )
      ).to.be.revertedWith("Pausable: paused");
    });

    it("Should allow owner to unpause", async function () {
      await marketplace.pause();
      await marketplace.unpause();
      expect(await marketplace.paused()).to.be.false;
    });

    it("Should not allow non-owner to pause", async function () {
      await expect(
        marketplace.connect(seller).pause()
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("Emergency Functions", function () {
    it("Should allow owner to recover stuck ERC20 tokens", async function () {
      // Send tokens to contract
      await token.transfer(marketplace.address, ethers.utils.parseEther("100"));
      
      const ownerBalanceBefore = await token.balanceOf(owner.address);
      const contractBalance = await token.balanceOf(marketplace.address);
      
      await marketplace.emergencyRecoverToken(token.address, contractBalance, owner.address);
      
      const ownerBalanceAfter = await token.balanceOf(owner.address);
      expect(ownerBalanceAfter.sub(ownerBalanceBefore)).to.equal(contractBalance);
      expect(await token.balanceOf(marketplace.address)).to.equal(0);
    });

    it("Should not allow non-owner to use emergency functions", async function () {
      await expect(
        marketplace.connect(seller).emergencyRecoverToken(token.address, 100, seller.address)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("View Functions", function () {
    it("Should access listing data", async function () {
      const ETH_ADDRESS = await marketplace.ETH_ADDRESS();
      const price = ethers.utils.parseEther("1");
      const expiration = Math.floor(Date.now() / 1000) + 3600;
      
      const tx = await marketplace.connect(seller).listNFT(nft.address, 1, ETH_ADDRESS, price, expiration);
      const receipt = await tx.wait();
      const newListingId = receipt.events.find(e => e.event === "NFTListed").args.listingId;
      
      const listing = await marketplace.listings(newListingId);
      expect(listing.seller).to.equal(seller.address);
      expect(listing.nftContract).to.equal(nft.address);
      expect(listing.tokenId).to.equal(1);
      expect(listing.price).to.equal(price);
      expect(listing.active).to.be.true;
    });
  });
});