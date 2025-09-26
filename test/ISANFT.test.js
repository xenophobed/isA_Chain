const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("ISANFT", function () {
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

    it("Should emit WhitelistStatusChanged events", async function () {
      await expect(isaNFT.addToWhitelist([addr1.address]))
        .to.emit(isaNFT, "WhitelistStatusChanged")
        .withArgs(addr1.address, true);
        
      await expect(isaNFT.removeFromWhitelist([addr1.address]))
        .to.emit(isaNFT, "WhitelistStatusChanged")
        .withArgs(addr1.address, false);
    });

    it("Should not allow non-admin to manage whitelist", async function () {
      await expect(
        isaNFT.connect(addr1).addToWhitelist([addr2.address])
      ).to.be.reverted;
      
      await expect(
        isaNFT.connect(addr1).removeFromWhitelist([addr2.address])
      ).to.be.reverted;
    });
  });

  describe("Public Minting", function () {
    beforeEach(async function () {
      await isaNFT.setMintingConfig(true, false); // Enable public minting
    });

    it("Should allow public minting when enabled", async function () {
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

    it("Should enforce max mint per transaction", async function () {
      const maxMintPerTx = await isaNFT.MAX_MINT_PER_TRANSACTION();
      const mintPrice = await isaNFT.mintPrice();
      
      await expect(
        isaNFT.connect(addr1).mint(maxMintPerTx.add(1), {
          value: mintPrice.mul(maxMintPerTx.add(1))
        })
      ).to.be.revertedWith("ISANFT: invalid quantity");
    });

    it("Should enforce max mint per wallet", async function () {
      const maxMintPerWallet = await isaNFT.MAX_MINT_PER_WALLET();
      const maxMintPerTx = await isaNFT.MAX_MINT_PER_TRANSACTION();
      const mintPrice = await isaNFT.mintPrice();
      
      // Mint in batches due to per-transaction limit
      const numBatches = Math.floor(maxMintPerWallet / maxMintPerTx);
      const remainder = maxMintPerWallet % maxMintPerTx;
      
      // Mint full batches
      for (let i = 0; i < numBatches; i++) {
        await isaNFT.connect(addr1).mint(maxMintPerTx, {
          value: mintPrice.mul(maxMintPerTx)
        });
      }
      
      // Mint remainder if any
      if (remainder > 0) {
        await isaNFT.connect(addr1).mint(remainder, {
          value: mintPrice.mul(remainder)
        });
      }
      
      // Should reject exceeding wallet limit
      await expect(
        isaNFT.connect(addr1).mint(1, { value: mintPrice })
      ).to.be.revertedWith("ISANFT: exceeds wallet limit");
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

    it("Should reveal collection", async function () {
      expect(await isaNFT.revealed()).to.be.false;
      
      await expect(isaNFT.reveal())
        .to.emit(isaNFT, "RevealStatusChanged")
        .withArgs(true);
        
      expect(await isaNFT.revealed()).to.be.true;
    });

    it("Should not allow non-metadata role to manage metadata", async function () {
      await expect(
        isaNFT.connect(addr1).setBaseURI("test")
      ).to.be.reverted;
      
      await expect(
        isaNFT.connect(addr1).reveal()
      ).to.be.reverted;
    });
  });

  describe("Pricing Configuration", function () {
    it("Should update mint prices", async function () {
      const newMintPrice = ethers.utils.parseEther("0.1");
      const newWhitelistPrice = ethers.utils.parseEther("0.08");
      
      await expect(isaNFT.setPrices(newMintPrice, newWhitelistPrice))
        .to.emit(isaNFT, "PriceUpdated")
        .withArgs(newMintPrice, newWhitelistPrice);
        
      expect(await isaNFT.mintPrice()).to.equal(newMintPrice);
      expect(await isaNFT.whitelistPrice()).to.equal(newWhitelistPrice);
    });

    it("Should update minting configuration", async function () {
      await expect(isaNFT.setMintingConfig(true, true))
        .to.emit(isaNFT, "MintingConfigUpdated")
        .withArgs(true, true);
        
      expect(await isaNFT.publicMintEnabled()).to.be.true;
      expect(await isaNFT.whitelistMintEnabled()).to.be.true;
    });

    it("Should not allow non-admin to update prices", async function () {
      await expect(
        isaNFT.connect(addr1).setPrices(
          ethers.utils.parseEther("0.1"),
          ethers.utils.parseEther("0.08")
        )
      ).to.be.reverted;
    });
  });

  describe("Royalty Management", function () {
    it("Should set royalty information", async function () {
      const newRecipient = addr1.address;
      const newFeeBps = 500; // 5%
      
      await expect(isaNFT.setRoyalty(newRecipient, newFeeBps))
        .to.emit(isaNFT, "RoyaltyUpdated")
        .withArgs(newRecipient, newFeeBps);
        
      expect(await isaNFT.royaltyRecipient()).to.equal(newRecipient);
      expect(await isaNFT.royaltyFeeBps()).to.equal(newFeeBps);
    });

    it("Should reject royalty fee over 10%", async function () {
      await expect(
        isaNFT.setRoyalty(addr1.address, 1001) // 10.01%
      ).to.be.revertedWith("ISANFT: royalty fee too high");
    });

    it("Should return correct royalty info", async function () {
      await isaNFT.adminMint(addr1.address, 1);
      
      const salePrice = ethers.utils.parseEther("1");
      const [recipient, royaltyAmount] = await isaNFT.royaltyInfo(1, salePrice);
      
      expect(recipient).to.equal(royaltyRecipient.address);
      // Default 7.5% of 1 ETH = 0.075 ETH
      expect(royaltyAmount).to.equal(ethers.utils.parseEther("0.075"));
    });

    it("Should not allow non-royalty role to set royalty", async function () {
      await expect(
        isaNFT.connect(addr1).setRoyalty(addr1.address, 500)
      ).to.be.reverted;
    });
  });

  describe("Withdraw Management", function () {
    beforeEach(async function () {
      // Enable public minting and mint some tokens to generate funds
      await isaNFT.setMintingConfig(true, false);
      const mintPrice = await isaNFT.mintPrice();
      await isaNFT.connect(addr1).mint(2, { value: mintPrice.mul(2) });
    });

    it("Should withdraw funds with correct split", async function () {
      const contractBalance = await ethers.provider.getBalance(isaNFT.address);
      const treasuryBalanceBefore = await ethers.provider.getBalance(treasury.address);
      const artistBalanceBefore = await ethers.provider.getBalance(artist.address);
      
      await isaNFT.withdraw();
      
      const treasuryBalanceAfter = await ethers.provider.getBalance(treasury.address);
      const artistBalanceAfter = await ethers.provider.getBalance(artist.address);
      
      const expectedTreasuryAmount = contractBalance.mul(7000).div(10000); // 70%
      const expectedArtistAmount = contractBalance.mul(3000).div(10000); // 30%
      
      expect(treasuryBalanceAfter.sub(treasuryBalanceBefore)).to.equal(expectedTreasuryAmount);
      expect(artistBalanceAfter.sub(artistBalanceBefore)).to.equal(expectedArtistAmount);
    });

    it("Should update withdraw split", async function () {
      await expect(isaNFT.setWithdrawSplit(8000, 2000)) // 80% treasury, 20% artist
        .to.emit(isaNFT, "WithdrawSplitUpdated")
        .withArgs(8000, 2000);
        
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