const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("ISANFT - Minting & Whitelist", function () {
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

  describe("Deployment", function () {
    it("Should set the right name and symbol", async function () {
      expect(await isaNFT.name()).to.equal("isA Chain NFT");
      expect(await isaNFT.symbol()).to.equal("ISANFT");
    });

    it("Should set correct addresses", async function () {
      expect(await isaNFT.treasury()).to.equal(treasury.address);
      expect(await isaNFT.artist()).to.equal(artist.address);
      expect(await isaNFT.royaltyRecipient()).to.equal(royaltyRecipient.address);
    });

    it("Should grant correct roles to deployer", async function () {
      const DEFAULT_ADMIN_ROLE = await isaNFT.DEFAULT_ADMIN_ROLE();
      const MINTER_ROLE = await isaNFT.MINTER_ROLE();
      const PAUSER_ROLE = await isaNFT.PAUSER_ROLE();
      const METADATA_ROLE = await isaNFT.METADATA_ROLE();
      const ROYALTY_ROLE = await isaNFT.ROYALTY_ROLE();

      expect(await isaNFT.hasRole(DEFAULT_ADMIN_ROLE, owner.address)).to.be.true;
      expect(await isaNFT.hasRole(MINTER_ROLE, owner.address)).to.be.true;
      expect(await isaNFT.hasRole(PAUSER_ROLE, owner.address)).to.be.true;
      expect(await isaNFT.hasRole(METADATA_ROLE, owner.address)).to.be.true;
      expect(await isaNFT.hasRole(ROYALTY_ROLE, owner.address)).to.be.true;
    });

    it("Should set correct initial configuration", async function () {
      expect(await isaNFT.MAX_SUPPLY()).to.equal(10000);
      expect(await isaNFT.mintPrice()).to.equal(ethers.utils.parseEther("0.05"));
      expect(await isaNFT.whitelistPrice()).to.equal(ethers.utils.parseEther("0.03"));
      expect(await isaNFT.publicMintEnabled()).to.be.false;
      expect(await isaNFT.whitelistMintEnabled()).to.be.false;
      expect(await isaNFT.revealed()).to.be.false;
    });
  });

  describe("Admin Minting", function () {
    it("Should allow admin to mint tokens", async function () {
      await isaNFT.adminMint(addr1.address, 5);
      
      expect(await isaNFT.balanceOf(addr1.address)).to.equal(5);
      expect(await isaNFT.totalSupply()).to.equal(5);
      expect(await isaNFT.ownerOf(1)).to.equal(addr1.address);
      expect(await isaNFT.ownerOf(5)).to.equal(addr1.address);
    });

    it("Should not allow non-admin to mint", async function () {
      await expect(
        isaNFT.connect(addr1).adminMint(addr1.address, 1)
      ).to.be.reverted;
    });

    it("Should not exceed max supply", async function () {
      const maxSupply = await isaNFT.MAX_SUPPLY();
      
      await expect(
        isaNFT.adminMint(addr1.address, maxSupply.add(1))
      ).to.be.revertedWith("ISANFT: exceeds max supply");
    });

    it("Should emit BatchMinted event", async function () {
      await expect(isaNFT.adminMint(addr1.address, 3))
        .to.emit(isaNFT, "BatchMinted")
        .withArgs(addr1.address, [1, 2, 3]);
    });
  });

  describe("Whitelist Management", function () {
    it("Should add addresses to whitelist", async function () {
      const addresses = [addr1.address, addr2.address];
      
      await isaNFT.addToWhitelist(addresses);
      
      expect(await isaNFT.isWhitelisted(addr1.address)).to.be.true;
      expect(await isaNFT.isWhitelisted(addr2.address)).to.be.true;
      expect(await isaNFT.whitelistCount()).to.equal(2);
    });

    it("Should remove addresses from whitelist", async function () {
      const addresses = [addr1.address, addr2.address];
      
      await isaNFT.addToWhitelist(addresses);
      await isaNFT.removeFromWhitelist([addr1.address]);
      
      expect(await isaNFT.isWhitelisted(addr1.address)).to.be.false;
      expect(await isaNFT.isWhitelisted(addr2.address)).to.be.true;
      expect(await isaNFT.whitelistCount()).to.equal(1);
    });

    it("Should not allow non-admin to manage whitelist", async function () {
      await expect(
        isaNFT.connect(addr1).addToWhitelist([addr2.address])
      ).to.be.reverted;
    });

    it("Should emit WhitelistUpdated event", async function () {
      await expect(isaNFT.addToWhitelist([addr1.address]))
        .to.emit(isaNFT, "WhitelistUpdated")
        .withArgs(addr1.address, true);
    });
  });

  describe("Public Minting", function () {
    beforeEach(async function () {
      await isaNFT.setMintingConfig(true, false); // Enable public minting
    });

    it("Should allow public minting with correct payment", async function () {
      const mintPrice = await isaNFT.mintPrice();
      const quantity = 2;
      
      await isaNFT.connect(addr1).mint(quantity, {
        value: mintPrice.mul(quantity)
      });
      
      expect(await isaNFT.balanceOf(addr1.address)).to.equal(quantity);
      expect(await isaNFT.walletMints(addr1.address)).to.equal(quantity);
    });

    it("Should reject insufficient payment", async function () {
      const mintPrice = await isaNFT.mintPrice();
      
      await expect(
        isaNFT.connect(addr1).mint(1, { value: mintPrice.sub(1) })
      ).to.be.revertedWith("ISANFT: insufficient payment");
    });

    it("Should refund excess payment", async function () {
      const mintPrice = await isaNFT.mintPrice();
      const excess = ethers.utils.parseEther("0.1");
      
      const balanceBefore = await addr1.getBalance();
      const tx = await isaNFT.connect(addr1).mint(1, {
        value: mintPrice.add(excess),
        gasPrice: ethers.utils.parseUnits("10", "gwei")
      });
      const receipt = await tx.wait();
      const gasUsed = receipt.gasUsed.mul(receipt.effectiveGasPrice);
      const balanceAfter = await addr1.getBalance();
      
      expect(balanceBefore.sub(balanceAfter).sub(gasUsed)).to.equal(mintPrice);
    });

    it("Should enforce max per wallet limit", async function () {
      const mintPrice = await isaNFT.mintPrice();
      const maxPerWallet = await isaNFT.MAX_PER_WALLET();
      
      // Mint up to limit
      await isaNFT.connect(addr1).mint(maxPerWallet, {
        value: mintPrice.mul(maxPerWallet)
      });
      
      // Try to mint one more
      await expect(
        isaNFT.connect(addr1).mint(1, { value: mintPrice })
      ).to.be.revertedWith("ISANFT: exceeds max per wallet");
    });

    it("Should not allow minting when disabled", async function () {
      await isaNFT.setMintingConfig(false, false); // Disable public minting
      const mintPrice = await isaNFT.mintPrice();
      
      await expect(
        isaNFT.connect(addr1).mint(1, { value: mintPrice })
      ).to.be.revertedWith("ISANFT: public minting not enabled");
    });
  });

  describe("Whitelist Minting", function () {
    beforeEach(async function () {
      await isaNFT.setMintingConfig(false, true); // Enable whitelist minting
      await isaNFT.addToWhitelist([addr1.address]);
    });

    it("Should allow whitelist minting when enabled", async function () {
      const whitelistPrice = await isaNFT.whitelistPrice();
      const quantity = 2;
      
      await isaNFT.connect(addr1).whitelistMint(quantity, {
        value: whitelistPrice.mul(quantity)
      });
      
      expect(await isaNFT.balanceOf(addr1.address)).to.equal(quantity);
      expect(await isaNFT.walletMints(addr1.address)).to.equal(quantity);
    });

    it("Should reject non-whitelisted addresses", async function () {
      const whitelistPrice = await isaNFT.whitelistPrice();
      
      await expect(
        isaNFT.connect(addr2).whitelistMint(1, { value: whitelistPrice })
      ).to.be.revertedWith("ISANFT: not whitelisted");
    });

    it("Should reject insufficient payment", async function () {
      const whitelistPrice = await isaNFT.whitelistPrice();
      
      await expect(
        isaNFT.connect(addr1).whitelistMint(1, { value: whitelistPrice.sub(1) })
      ).to.be.revertedWith("ISANFT: insufficient payment");
    });

    it("Should not allow minting when disabled", async function () {
      await isaNFT.setMintingConfig(false, false); // Disable whitelist minting
      const whitelistPrice = await isaNFT.whitelistPrice();
      
      await expect(
        isaNFT.connect(addr1).whitelistMint(1, { value: whitelistPrice })
      ).to.be.revertedWith("ISANFT: whitelist minting not enabled");
    });
  });
});
