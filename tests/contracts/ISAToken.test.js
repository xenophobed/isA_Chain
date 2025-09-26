const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("ISAToken", function () {
  let isaToken;
  let owner, treasury, addr1, addr2, addr3;
  
  beforeEach(async function () {
    [owner, treasury, addr1, addr2, addr3] = await ethers.getSigners();
    
    const ISAToken = await ethers.getContractFactory("ISAToken");
    isaToken = await ISAToken.deploy(treasury.address);
    await isaToken.deployed();
  });

  describe("Deployment", function () {
    it("Should set the right name and symbol", async function () {
      expect(await isaToken.name()).to.equal("isA Chain Token");
      expect(await isaToken.symbol()).to.equal("ISA");
    });

    it("Should mint initial supply to treasury", async function () {
      const initialSupply = ethers.utils.parseEther("1000000000"); // 1 billion
      expect(await isaToken.balanceOf(treasury.address)).to.equal(initialSupply);
      expect(await isaToken.totalSupply()).to.equal(initialSupply);
    });

    it("Should set correct maximum supply", async function () {
      const maxSupply = ethers.utils.parseEther("10000000000"); // 10 billion
      expect(await isaToken.MAX_SUPPLY()).to.equal(maxSupply);
    });

    it("Should grant correct roles to deployer", async function () {
      const DEFAULT_ADMIN_ROLE = await isaToken.DEFAULT_ADMIN_ROLE();
      const MINTER_ROLE = await isaToken.MINTER_ROLE();
      const PAUSER_ROLE = await isaToken.PAUSER_ROLE();
      const BURNER_ROLE = await isaToken.BURNER_ROLE();

      expect(await isaToken.hasRole(DEFAULT_ADMIN_ROLE, owner.address)).to.be.true;
      expect(await isaToken.hasRole(MINTER_ROLE, owner.address)).to.be.true;
      expect(await isaToken.hasRole(PAUSER_ROLE, owner.address)).to.be.true;
      expect(await isaToken.hasRole(BURNER_ROLE, owner.address)).to.be.true;
    });
  });

  describe("Minting", function () {
    it("Should allow minter to mint new tokens", async function () {
      const mintAmount = ethers.utils.parseEther("1000");
      await isaToken.mint(addr1.address, mintAmount);
      
      expect(await isaToken.balanceOf(addr1.address)).to.equal(mintAmount);
    });

    it("Should not exceed maximum supply", async function () {
      const maxSupply = await isaToken.MAX_SUPPLY();
      const currentSupply = await isaToken.totalSupply();
      const excessAmount = maxSupply.sub(currentSupply).add(1);
      
      await expect(
        isaToken.mint(addr1.address, excessAmount)
      ).to.be.revertedWith("ISAToken: exceeds max supply");
    });

    it("Should not allow non-minter to mint", async function () {
      const mintAmount = ethers.utils.parseEther("1000");
      
      await expect(
        isaToken.connect(addr1).mint(addr1.address, mintAmount)
      ).to.be.reverted;
    });
  });

  describe("Burning", function () {
    beforeEach(async function () {
      const mintAmount = ethers.utils.parseEther("1000");
      await isaToken.mint(addr1.address, mintAmount);
    });

    it("Should allow burner to burn tokens", async function () {
      const burnAmount = ethers.utils.parseEther("500");
      const initialBalance = await isaToken.balanceOf(addr1.address);
      
      // Approve the contract to burn tokens from addr1
      await isaToken.connect(addr1).approve(owner.address, burnAmount);
      await isaToken.burnFrom(addr1.address, burnAmount);
      
      expect(await isaToken.balanceOf(addr1.address)).to.equal(
        initialBalance.sub(burnAmount)
      );
    });

    it("Should not allow non-burner to burn tokens", async function () {
      const burnAmount = ethers.utils.parseEther("500");
      
      await expect(
        isaToken.connect(addr1).burnFrom(addr1.address, burnAmount)
      ).to.be.reverted;
    });
  });

  describe("Pausing", function () {
    it("Should allow pauser to pause transfers", async function () {
      await isaToken.pause();
      expect(await isaToken.paused()).to.be.true;

      await expect(
        isaToken.connect(treasury).transfer(addr1.address, 100)
      ).to.be.revertedWith("ERC20Pausable: token transfer while paused");
    });

    it("Should allow pauser to unpause", async function () {
      await isaToken.pause();
      await isaToken.unpause();
      expect(await isaToken.paused()).to.be.false;

      await expect(
        isaToken.connect(treasury).transfer(addr1.address, 100)
      ).to.not.be.reverted;
    });

    it("Should not allow non-pauser to pause", async function () {
      await expect(
        isaToken.connect(addr1).pause()
      ).to.be.reverted;
    });
  });

  describe("Vesting", function () {
    beforeEach(async function () {
      // Transfer some tokens from treasury to owner for vesting tests
      const vestingAmount = ethers.utils.parseEther("10000");
      await isaToken.connect(treasury).transfer(owner.address, vestingAmount);
    });

    it("Should create vesting schedule", async function () {
      const totalAmount = ethers.utils.parseEther("1000");
      const start = Math.floor(Date.now() / 1000);
      const duration = 365 * 24 * 60 * 60; // 1 year
      const cliffDuration = 30 * 24 * 60 * 60; // 30 days

      // Owner creates vesting from their balance (they have admin role)
      await isaToken.createVestingSchedule(
        addr1.address,
        totalAmount,
        start,
        duration,
        cliffDuration,
        true // revokable
      );

      const schedule = await isaToken.getVestingSchedule(addr1.address);
      expect(schedule.totalAmount).to.equal(totalAmount);
      expect(schedule.start).to.equal(start);
      expect(schedule.duration).to.equal(duration);
      expect(schedule.cliffDuration).to.equal(cliffDuration);
      expect(schedule.revokable).to.be.true;
    });

    it("Should calculate releasable amount correctly", async function () {
      const totalAmount = ethers.utils.parseEther("1000");
      const start = Math.floor(Date.now() / 1000) - 60; // Started 1 minute ago
      const duration = 365 * 24 * 60 * 60; // 1 year
      const cliffDuration = 0; // No cliff

      await isaToken.createVestingSchedule(
        addr1.address,
        totalAmount,
        start,
        duration,
        cliffDuration,
        false
      );

      const releasableAmount = await isaToken.releasableAmount(addr1.address);
      expect(releasableAmount).to.be.gt(0);
    });

    it("Should release vested tokens", async function () {
      const totalAmount = ethers.utils.parseEther("1000");
      const start = Math.floor(Date.now() / 1000) - 100; // Started recently
      const duration = 1000; // Short duration for testing
      const cliffDuration = 0;

      await isaToken.createVestingSchedule(
        addr1.address,
        totalAmount,
        start,
        duration,
        cliffDuration,
        false
      );

      const initialBalance = await isaToken.balanceOf(addr1.address);
      await isaToken.release(addr1.address);
      const finalBalance = await isaToken.balanceOf(addr1.address);

      expect(finalBalance).to.be.gt(initialBalance);
    });
  });

  describe("Governance (ERC20Votes)", function () {
    it("Should support delegation", async function () {
      const amount = ethers.utils.parseEther("1000");
      await isaToken.mint(addr1.address, amount);
      
      await isaToken.connect(addr1).delegate(addr2.address);
      
      expect(await isaToken.delegates(addr1.address)).to.equal(addr2.address);
      expect(await isaToken.getVotes(addr2.address)).to.equal(amount);
    });

    it("Should track voting power correctly", async function () {
      const amount = ethers.utils.parseEther("1000");
      await isaToken.mint(addr1.address, amount);
      
      // Self-delegate to activate voting power
      await isaToken.connect(addr1).delegate(addr1.address);
      
      expect(await isaToken.getVotes(addr1.address)).to.equal(amount);
    });
  });

  describe("Permit (ERC20Permit)", function () {
    it("Should support permit functionality", async function () {
      const amount = ethers.utils.parseEther("1000");
      await isaToken.mint(addr1.address, amount);
      
      const nonce = await isaToken.nonces(addr1.address);
      expect(nonce).to.equal(0);
      
      // This is a basic check - full permit testing would require signing
      expect(await isaToken.DOMAIN_SEPARATOR()).to.not.equal(ethers.constants.HashZero);
    });
  });

  describe("Access Control", function () {
    it("Should allow admin to grant roles", async function () {
      const MINTER_ROLE = await isaToken.MINTER_ROLE();
      
      await isaToken.grantRole(MINTER_ROLE, addr1.address);
      expect(await isaToken.hasRole(MINTER_ROLE, addr1.address)).to.be.true;
    });

    it("Should allow admin to revoke roles", async function () {
      const MINTER_ROLE = await isaToken.MINTER_ROLE();
      
      await isaToken.grantRole(MINTER_ROLE, addr1.address);
      await isaToken.revokeRole(MINTER_ROLE, addr1.address);
      
      expect(await isaToken.hasRole(MINTER_ROLE, addr1.address)).to.be.false;
    });
  });
});