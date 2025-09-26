const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("SimpleToken", function () {
  let SimpleToken;
  let simpleToken;
  let owner;
  let addr1;
  let addr2;

  beforeEach(async function () {
    [owner, addr1, addr2] = await ethers.getSigners();

    SimpleToken = await ethers.getContractFactory("SimpleToken");
    simpleToken = await SimpleToken.deploy();
  });

  describe("Deployment", function () {
    it("Should set the right owner", async function () {
      expect(await simpleToken.owner()).to.equal(owner.address);
    });

    it("Should assign the total supply of tokens to the owner", async function () {
      const ownerBalance = await simpleToken.balanceOf(owner.address);
      expect(await simpleToken.totalSupply()).to.equal(ownerBalance);
    });

    it("Should have correct name and symbol", async function () {
      expect(await simpleToken.name()).to.equal("Simple Token");
      expect(await simpleToken.symbol()).to.equal("SIMPLE");
    });
  });

  describe("Transactions", function () {
    it("Should transfer tokens between accounts", async function () {
      await simpleToken.transfer(addr1.address, 50);
      const addr1Balance = await simpleToken.balanceOf(addr1.address);
      expect(addr1Balance).to.equal(50);

      const ownerBalance = await simpleToken.balanceOf(owner.address);
      expect(ownerBalance).to.equal(ethers.utils.parseEther("1000000").sub(50));
    });

    it("Should fail if sender doesn't have enough tokens", async function () {
      const initialOwnerBalance = await simpleToken.balanceOf(owner.address);
      
      await expect(
        simpleToken.connect(addr1).transfer(owner.address, 1)
      ).to.be.revertedWith("ERC20: transfer amount exceeds balance");

      expect(await simpleToken.balanceOf(owner.address)).to.equal(
        initialOwnerBalance
      );
    });

    it("Should update balances after transfers", async function () {
      const initialOwnerBalance = await simpleToken.balanceOf(owner.address);

      await simpleToken.transfer(addr1.address, 100);
      await simpleToken.transfer(addr2.address, 50);

      const finalOwnerBalance = await simpleToken.balanceOf(owner.address);
      expect(finalOwnerBalance).to.equal(initialOwnerBalance.sub(150));

      const addr1Balance = await simpleToken.balanceOf(addr1.address);
      expect(addr1Balance).to.equal(100);

      const addr2Balance = await simpleToken.balanceOf(addr2.address);
      expect(addr2Balance).to.equal(50);
    });
  });

  describe("Minting", function () {
    it("Should allow owner to mint new tokens", async function () {
      const initialSupply = await simpleToken.totalSupply();
      await simpleToken.mint(addr1.address, 1000);
      
      const newSupply = await simpleToken.totalSupply();
      expect(newSupply).to.equal(initialSupply.add(1000));
      
      const addr1Balance = await simpleToken.balanceOf(addr1.address);
      expect(addr1Balance).to.equal(1000);
    });

    it("Should not allow non-owner to mint tokens", async function () {
      await expect(
        simpleToken.connect(addr1).mint(addr1.address, 1000)
      ).to.be.revertedWith("Only owner can mint");
    });
  });
});