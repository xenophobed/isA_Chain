const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("ISANFT - Lifecycle & Management", function () {
  let isaNFT;
  let owner, treasury, artist, royaltyRecipient, addr1, addr2, addr3;
  
  beforeEach(async function () {
    [owner, treasury, artist, royaltyRecipient, addr1, addr2, addr3] = await ethers.getSigners();
    
    const ISANFT = await ethers.getContractFactory("ISANFT");
    isaNFT = await ISANFT.deploy(
      treasury.address,
      artist.address,
      royaltyRecipient.address
    );
    await isaNFT.deployed();
  });

  describe("Metadata Management", function () {
    beforeEach(async function () {
      await isaNFT.adminMint(addr1.address, 1);
    });

    it("Should set base URI", async function () {
      const baseURI = "https://api.isachain.io/metadata/";
      await isaNFT.setBaseURI(baseURI);
      await isaNFT.reveal();
      
      const tokenURI = await isaNFT.tokenURI(1);
      expect(tokenURI).to.equal(baseURI + "1.json");
    });

    it("Should set hidden metadata URI", async function () {
      const hiddenURI = "https://api.isachain.io/hidden.json";
      await isaNFT.setHiddenMetadataURI(hiddenURI);
      
      // Should return hidden URI when not revealed
      const tokenURI = await isaNFT.tokenURI(1);
      expect(tokenURI).to.equal(hiddenURI);
    });

    it("Should set individual token URI", async function () {
      const customURI = "https://custom.uri/token.json";
      await isaNFT.setTokenURI(1, customURI);
      
      const tokenURI = await isaNFT.tokenURI(1);
      expect(tokenURI).to.equal(customURI);
    });

    it("Should emit Revealed event", async function () {
      await expect(isaNFT.reveal())
        .to.emit(isaNFT, "Revealed");
    });

    it("Should not allow non-metadata role to update URIs", async function () {
      await expect(
        isaNFT.connect(addr1).setBaseURI("https://new.uri/")
      ).to.be.reverted;
    });
  });

  describe("Royalties (EIP-2981)", function () {
    it("Should return correct royalty info", async function () {
      const salePrice = ethers.utils.parseEther("1.0");
      const expectedRoyalty = salePrice.mul(500).div(10000); // 5%
      
      const [receiver, royaltyAmount] = await isaNFT.royaltyInfo(1, salePrice);
      
      expect(receiver).to.equal(royaltyRecipient.address);
      expect(royaltyAmount).to.equal(expectedRoyalty);
    });

    it("Should allow royalty admin to update recipient", async function () {
      await isaNFT.setRoyaltyRecipient(addr1.address);
      
      const [receiver] = await isaNFT.royaltyInfo(1, ethers.utils.parseEther("1.0"));
      expect(receiver).to.equal(addr1.address);
    });

    it("Should allow royalty admin to update fee", async function () {
      await isaNFT.setRoyaltyFee(1000); // 10%
      
      const salePrice = ethers.utils.parseEther("1.0");
      const expectedRoyalty = salePrice.mul(1000).div(10000);
      
      const [, royaltyAmount] = await isaNFT.royaltyInfo(1, salePrice);
      expect(royaltyAmount).to.equal(expectedRoyalty);
    });

    it("Should reject royalty fee above max", async function () {
      await expect(
        isaNFT.setRoyaltyFee(1001) // Above 10%
      ).to.be.revertedWith("ISANFT: royalty fee too high");
    });

    it("Should not allow non-royalty role to update", async function () {
      await expect(
        isaNFT.connect(addr1).setRoyaltyFee(500)
      ).to.be.reverted;
    });
  });

  describe("Withdrawals", function () {
    beforeEach(async function () {
      // Add funds to contract via public mint
      await isaNFT.setMintingConfig(true, false);
      const mintPrice = await isaNFT.mintPrice();
      await isaNFT.connect(addr1).mint(2, { value: mintPrice.mul(2) });
    });

    it("Should distribute funds according to shares", async function () {
      const contractBalance = await ethers.provider.getBalance(isaNFT.address);
      const treasuryBalanceBefore = await ethers.provider.getBalance(treasury.address);
      const artistBalanceBefore = await ethers.provider.getBalance(artist.address);
      
      await isaNFT.withdraw();
      
      const treasuryBalanceAfter = await ethers.provider.getBalance(treasury.address);
      const artistBalanceAfter = await ethers.provider.getBalance(artist.address);
      
      // Default: 70% treasury, 30% artist
      const expectedTreasuryAmount = contractBalance.mul(7000).div(10000);
      const expectedArtistAmount = contractBalance.mul(3000).div(10000);
      
      expect(treasuryBalanceAfter.sub(treasuryBalanceBefore)).to.equal(expectedTreasuryAmount);
      expect(artistBalanceAfter.sub(artistBalanceBefore)).to.equal(expectedArtistAmount);
    });

    it("Should allow updating withdraw shares", async function () {
      await isaNFT.setWithdrawSplit(8000, 2000); // 80/20 split
      
      expect(await isaNFT.treasuryShare()).to.equal(8000);
      expect(await isaNFT.artistShare()).to.equal(2000);
    });

    it("Should reject invalid split percentages", async function () {
      await expect(
        isaNFT.setWithdrawSplit(8000, 3000) // Total 110%
      ).to.be.revertedWith("ISANFT: shares must equal 100%");
    });

    it("Should not allow non-admin to withdraw", async function () {
      await expect(
        isaNFT.connect(addr1).withdraw()
      ).to.be.reverted;
    });
  });

  describe("Pausing", function () {
    it("Should allow pauser to pause transfers", async function () {
      await isaNFT.adminMint(addr1.address, 1);
      
      await isaNFT.pause();
      expect(await isaNFT.paused()).to.be.true;
      
      await expect(
        isaNFT.connect(addr1).transferFrom(addr1.address, addr2.address, 1)
      ).to.be.revertedWith("ERC721Pausable: token transfer while paused");
    });

    it("Should allow pauser to unpause", async function () {
      await isaNFT.pause();
      await isaNFT.unpause();
      expect(await isaNFT.paused()).to.be.false;
    });

    it("Should not allow non-pauser to pause", async function () {
      await expect(
        isaNFT.connect(addr1).pause()
      ).to.be.reverted;
    });
  });

  describe("Burning", function () {
    beforeEach(async function () {
      await isaNFT.adminMint(addr1.address, 3);
    });

    it("Should allow token owner to burn", async function () {
      expect(await isaNFT.balanceOf(addr1.address)).to.equal(3);
      
      await isaNFT.connect(addr1).burn(1);
      
      expect(await isaNFT.balanceOf(addr1.address)).to.equal(2);
      await expect(isaNFT.ownerOf(1)).to.be.revertedWith("ERC721: invalid token ID");
    });

    it("Should not allow non-owner to burn", async function () {
      await expect(
        isaNFT.connect(addr2).burn(1)
      ).to.be.revertedWith("ERC721: caller is not token owner or approved");
    });
  });

  describe("Collection Information", function () {
    it("Should return correct collection info", async function () {
      await isaNFT.adminMint(addr1.address, 5);
      await isaNFT.addToWhitelist([addr1.address, addr2.address]);
      
      const [
        currentSupply,
        maxSupply,
        publicPrice,
        whitelistPriceValue,
        publicEnabled,
        whitelistEnabled,
        isRevealed,
        whitelistTotal
      ] = await isaNFT.getCollectionInfo();
      
      expect(currentSupply).to.equal(5);
      expect(maxSupply).to.equal(10000);
      expect(publicPrice).to.equal(ethers.utils.parseEther("0.05"));
      expect(whitelistPriceValue).to.equal(ethers.utils.parseEther("0.03"));
      expect(publicEnabled).to.be.false;
      expect(whitelistEnabled).to.be.false;
      expect(isRevealed).to.be.false;
      expect(whitelistTotal).to.equal(2);
    });
  });

  describe("Access Control", function () {
    it("Should allow admin to grant roles", async function () {
      const MINTER_ROLE = await isaNFT.MINTER_ROLE();
      
      await isaNFT.grantRole(MINTER_ROLE, addr1.address);
      expect(await isaNFT.hasRole(MINTER_ROLE, addr1.address)).to.be.true;
    });

    it("Should allow admin to revoke roles", async function () {
      const MINTER_ROLE = await isaNFT.MINTER_ROLE();
      
      await isaNFT.grantRole(MINTER_ROLE, addr1.address);
      await isaNFT.revokeRole(MINTER_ROLE, addr1.address);
      
      expect(await isaNFT.hasRole(MINTER_ROLE, addr1.address)).to.be.false;
    });
  });
});
