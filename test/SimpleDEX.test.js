const { expect } = require("chai");
const { ethers } = require("hardhat");

describe("SimpleDEX", function () {
  let dex, tokenA, tokenB, tokenC;
  let owner, feeCollector, trader, liquidityProvider;
  let pairId;
  
  beforeEach(async function () {
    [owner, feeCollector, trader, liquidityProvider] = await ethers.getSigners();
    
    // Deploy test tokens
    const SimpleToken = await ethers.getContractFactory("SimpleToken");
    tokenA = await SimpleToken.deploy();
    await tokenA.deployed();
    
    tokenB = await SimpleToken.deploy();
    await tokenB.deployed();
    
    tokenC = await SimpleToken.deploy();
    await tokenC.deployed();
    
    // Deploy DEX
    const SimpleDEX = await ethers.getContractFactory("SimpleDEX");
    dex = await SimpleDEX.deploy(feeCollector.address);
    await dex.deployed();
    
    // Setup: Add token support
    await dex.setSupportedToken(tokenA.address, true);
    await dex.setSupportedToken(tokenB.address, true);
    await dex.setSupportedToken(tokenC.address, true);
    
    // Distribute tokens for testing
    const amount = ethers.utils.parseEther("10000");
    await tokenA.transfer(trader.address, amount);
    await tokenA.transfer(liquidityProvider.address, amount);
    await tokenB.transfer(trader.address, amount);
    await tokenB.transfer(liquidityProvider.address, amount);
    
    // Approve DEX to spend tokens
    await tokenA.connect(trader).approve(dex.address, ethers.constants.MaxUint256);
    await tokenA.connect(liquidityProvider).approve(dex.address, ethers.constants.MaxUint256);
    await tokenB.connect(trader).approve(dex.address, ethers.constants.MaxUint256);
    await tokenB.connect(liquidityProvider).approve(dex.address, ethers.constants.MaxUint256);
  });

  describe("Deployment", function () {
    it("Should set correct fee collector", async function () {
      expect(await dex.feeCollector()).to.equal(feeCollector.address);
    });

    it("Should set correct default fee rate", async function () {
      expect(await dex.defaultFeeRate()).to.equal(300); // 3%
    });

    it("Should set correct protocol fee rate", async function () {
      expect(await dex.protocolFeeRate()).to.equal(30); // 0.3%
    });

    it("Should set correct owner", async function () {
      expect(await dex.owner()).to.equal(owner.address);
    });

    it("Should set correct minimum liquidity", async function () {
      expect(await dex.minimumLiquidity()).to.equal(1000);
    });
  });

  describe("Token Support Management", function () {
    it("Should add supported token", async function () {
      const newToken = await (await ethers.getContractFactory("SimpleToken")).deploy();
      
      await expect(dex.setSupportedToken(newToken.address, true))
        .to.emit(dex, "TokenSupported")
        .withArgs(newToken.address, true);
        
      expect(await dex.supportedTokens(newToken.address)).to.be.true;
    });

    it("Should remove supported token", async function () {
      await dex.setSupportedToken(tokenA.address, false);
      expect(await dex.supportedTokens(tokenA.address)).to.be.false;
    });

    it("Should not allow non-owner to manage token support", async function () {
      await expect(
        dex.connect(trader).setSupportedToken(tokenA.address, false)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("Trading Pair Creation", function () {
    it("Should create trading pair", async function () {
      const feeRate = 250; // 2.5%
      
      const tx = await dex.createPair(tokenA.address, tokenB.address, feeRate);
      const receipt = await tx.wait();
      
      // Get pairId from event
      const pairCreatedEvent = receipt.events.find(e => e.event === "PairCreated");
      pairId = pairCreatedEvent.args.pairId;
      
      const pair = await dex.getPairInfo(pairId);
      expect(pair.feeRate).to.equal(feeRate);
      expect(pair.active).to.be.true;
    });

    it("Should not create pair with unsupported tokens", async function () {
      const newToken = await (await ethers.getContractFactory("SimpleToken")).deploy();
      
      await expect(
        dex.createPair(tokenA.address, newToken.address, 300)
      ).to.be.revertedWith("SimpleDEX: tokens not supported");
    });

    it("Should not create pair with identical tokens", async function () {
      await expect(
        dex.createPair(tokenA.address, tokenA.address, 300)
      ).to.be.revertedWith("SimpleDEX: identical tokens");
    });

    it("Should not create pair with excessive fee rate", async function () {
      await expect(
        dex.createPair(tokenA.address, tokenB.address, 1001) // > 10%
      ).to.be.revertedWith("SimpleDEX: fee rate too high");
    });

    it("Should not allow duplicate pair creation", async function () {
      await dex.createPair(tokenA.address, tokenB.address, 300);
      
      await expect(
        dex.createPair(tokenA.address, tokenB.address, 300)
      ).to.be.revertedWith("SimpleDEX: pair already exists");
    });

    it("Should not allow non-owner to create pairs", async function () {
      await expect(
        dex.connect(trader).createPair(tokenA.address, tokenB.address, 300)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("Liquidity Management", function () {
    beforeEach(async function () {
      // Create pair
      const tx = await dex.createPair(tokenA.address, tokenB.address, 300);
      const receipt = await tx.wait();
      
      // Get pairId from event
      const pairCreatedEvent = receipt.events.find(e => e.event === "PairCreated");
      pairId = pairCreatedEvent.args.pairId;
    });

    it("Should add initial liquidity", async function () {
      const amountA = ethers.utils.parseEther("100");
      const amountB = ethers.utils.parseEther("200");
      
      await expect(
        dex.connect(liquidityProvider).addLiquidity(pairId, amountA, amountB, 0, 0)
      ).to.emit(dex, "LiquidityAdded");
      
      const pair = await dex.getPairInfo(pairId);
      const position = await dex.getUserPosition(pairId, liquidityProvider.address);
      
      expect(pair.reserveA).to.be.gt(0);
      expect(pair.reserveB).to.be.gt(0);
      expect(pair.totalLiquidity).to.be.gt(0);
      expect(position.liquidity).to.be.gt(0);
    });

    it("Should add liquidity to existing pool", async function () {
      // First liquidity addition
      const initialA = ethers.utils.parseEther("100");
      const initialB = ethers.utils.parseEther("200");
      await dex.connect(liquidityProvider).addLiquidity(pairId, initialA, initialB, 0, 0);
      
      // Second liquidity addition
      const additionalA = ethers.utils.parseEther("50");
      const additionalB = ethers.utils.parseEther("100");
      
      const pairBefore = await dex.getPairInfo(pairId);
      const positionBefore = await dex.getUserPosition(pairId, trader.address);
      
      await dex.connect(trader).addLiquidity(pairId, additionalA, additionalB, 0, 0);
      
      const pairAfter = await dex.getPairInfo(pairId);
      const positionAfter = await dex.getUserPosition(pairId, trader.address);
      
      expect(pairAfter.reserveA).to.be.gt(pairBefore.reserveA);
      expect(pairAfter.reserveB).to.be.gt(pairBefore.reserveB);
      expect(positionAfter.liquidity).to.be.gt(positionBefore.liquidity);
    });

    it("Should remove liquidity", async function () {
      // Add liquidity first
      const amountA = ethers.utils.parseEther("100");
      const amountB = ethers.utils.parseEther("200");
      await dex.connect(liquidityProvider).addLiquidity(pairId, amountA, amountB, 0, 0);
      
      const position = await dex.getUserPosition(pairId, liquidityProvider.address);
      const liquidityToRemove = position.liquidity.div(2); // Remove half
      
      const balanceABefore = await tokenA.balanceOf(liquidityProvider.address);
      const balanceBBefore = await tokenB.balanceOf(liquidityProvider.address);
      
      await expect(
        dex.connect(liquidityProvider).removeLiquidity(pairId, liquidityToRemove, 0, 0)
      ).to.emit(dex, "LiquidityRemoved");
      
      const balanceAAfter = await tokenA.balanceOf(liquidityProvider.address);
      const balanceBAfter = await tokenB.balanceOf(liquidityProvider.address);
      
      expect(balanceAAfter).to.be.gt(balanceABefore);
      expect(balanceBAfter).to.be.gt(balanceBBefore);
    });

    it("Should not allow adding liquidity to non-existent pair", async function () {
      const fakePairId = ethers.utils.keccak256(ethers.utils.toUtf8Bytes("fake"));
      
      await expect(
        dex.connect(liquidityProvider).addLiquidity(
          fakePairId, 
          ethers.utils.parseEther("100"), 
          ethers.utils.parseEther("200"), 
          0, 
          0
        )
      ).to.be.revertedWith("SimpleDEX: pair not active");
    });

    it("Should not allow removing more liquidity than owned", async function () {
      const amountA = ethers.utils.parseEther("100");
      const amountB = ethers.utils.parseEther("200");
      await dex.connect(liquidityProvider).addLiquidity(pairId, amountA, amountB, 0, 0);
      
      const position = await dex.getUserPosition(pairId, liquidityProvider.address);
      const excessiveLiquidity = position.liquidity.add(1);
      
      await expect(
        dex.connect(liquidityProvider).removeLiquidity(pairId, excessiveLiquidity, 0, 0)
      ).to.be.revertedWith("SimpleDEX: insufficient liquidity");
    });
  });

  describe("Token Swapping", function () {
    beforeEach(async function () {
      // Create pair and add liquidity
      const tx = await dex.createPair(tokenA.address, tokenB.address, 300);
      const receipt = await tx.wait();
      
      // Get pairId from event
      const pairCreatedEvent = receipt.events.find(e => e.event === "PairCreated");
      pairId = pairCreatedEvent.args.pairId;
      
      // Add significant liquidity
      const amountA = ethers.utils.parseEther("1000");
      const amountB = ethers.utils.parseEther("2000");
      await dex.connect(liquidityProvider).addLiquidity(pairId, amountA, amountB, 0, 0);
    });

    it("Should swap tokens", async function () {
      const swapAmount = ethers.utils.parseEther("10");
      
      const balanceABefore = await tokenA.balanceOf(trader.address);
      const balanceBBefore = await tokenB.balanceOf(trader.address);
      
      await expect(
        dex.connect(trader).swap(pairId, tokenA.address, swapAmount, 0)
      ).to.emit(dex, "TokensSwapped");
      
      const balanceAAfter = await tokenA.balanceOf(trader.address);
      const balanceBAfter = await tokenB.balanceOf(trader.address);
      
      expect(balanceAAfter).to.equal(balanceABefore.sub(swapAmount));
      expect(balanceBAfter).to.be.gt(balanceBBefore);
    });

    it("Should calculate correct swap amounts", async function () {
      const swapAmount = ethers.utils.parseEther("10");
      const pairBefore = await dex.getPairInfo(pairId);
      
      await dex.connect(trader).swap(pairId, tokenA.address, swapAmount, 0);
      
      const pairAfter = await dex.getPairInfo(pairId);
      
      // Check which token corresponds to which reserve
      const pair = await dex.getPairInfo(pairId);
      const isTokenAFirst = pair.tokenA.toLowerCase() === tokenA.address.toLowerCase();
      
      if (isTokenAFirst) {
        // tokenA is reserveA, so swapping tokenA should increase reserveA
        expect(pairAfter.reserveA).to.be.gt(pairBefore.reserveA);
        expect(pairAfter.reserveB).to.be.lt(pairBefore.reserveB);
      } else {
        // tokenA is reserveB, so swapping tokenA should increase reserveB
        expect(pairAfter.reserveB).to.be.gt(pairBefore.reserveB);
        expect(pairAfter.reserveA).to.be.lt(pairBefore.reserveA);
      }
      
      // Total liquidity should remain the same
      expect(pairAfter.totalLiquidity).to.equal(pairBefore.totalLiquidity);
    });

    it("Should collect protocol fees", async function () {
      const swapAmount = ethers.utils.parseEther("100");
      
      const feeCollectorBalanceBefore = await tokenA.balanceOf(feeCollector.address);
      
      await dex.connect(trader).swap(pairId, tokenA.address, swapAmount, 0);
      
      const feeCollectorBalanceAfter = await tokenA.balanceOf(feeCollector.address);
      
      // Fee collector should receive protocol fees
      expect(feeCollectorBalanceAfter).to.be.gt(feeCollectorBalanceBefore);
    });

    it("Should reject swap with insufficient output", async function () {
      const swapAmount = ethers.utils.parseEther("10");
      const unrealisticMinOutput = ethers.utils.parseEther("1000"); // Way too high
      
      await expect(
        dex.connect(trader).swap(pairId, tokenA.address, swapAmount, unrealisticMinOutput)
      ).to.be.revertedWith("SimpleDEX: insufficient output amount");
    });

    it("Should not allow swap with unsupported token", async function () {
      const newToken = await (await ethers.getContractFactory("SimpleToken")).deploy();
      const swapAmount = ethers.utils.parseEther("10");
      
      await expect(
        dex.connect(trader).swap(pairId, newToken.address, swapAmount, 0)
      ).to.be.revertedWith("SimpleDEX: invalid token");
    });

    it("Should not allow zero amount swap", async function () {
      await expect(
        dex.connect(trader).swap(pairId, tokenA.address, 0, 0)
      ).to.be.revertedWith("SimpleDEX: insufficient input amount");
    });
  });

  describe("Fee Management", function () {
    beforeEach(async function () {
      const tx = await dex.createPair(tokenA.address, tokenB.address, 300);
      const receipt = await tx.wait();
      
      // Get pairId from event
      const pairCreatedEvent = receipt.events.find(e => e.event === "PairCreated");
      pairId = pairCreatedEvent.args.pairId;
    });

    it("Should update protocol fee rate", async function () {
      const newRate = 50; // 0.5%
      
      await expect(dex.setProtocolFeeRate(newRate))
        .to.emit(dex, "ProtocolFeeUpdated")
        .withArgs(30, newRate);
        
      expect(await dex.protocolFeeRate()).to.equal(newRate);
    });

    it("Should update pair fee rate", async function () {
      const newRate = 400; // 4%
      
      await expect(dex.setPairFeeRate(pairId, newRate))
        .to.emit(dex, "FeeRateUpdated")
        .withArgs(pairId, 300, newRate);
        
      const pair = await dex.getPairInfo(pairId);
      expect(pair.feeRate).to.equal(newRate);
    });

    it("Should not allow excessive protocol fee rate", async function () {
      await expect(
        dex.setProtocolFeeRate(501) // > 5%
      ).to.be.revertedWith("SimpleDEX: protocol fee too high");
    });

    it("Should not allow excessive pair fee rate", async function () {
      await expect(
        dex.setPairFeeRate(pairId, 1001) // > 10%
      ).to.be.revertedWith("SimpleDEX: fee rate too high");
    });

    it("Should not allow non-owner to update fees", async function () {
      await expect(
        dex.connect(trader).setProtocolFeeRate(50)
      ).to.be.revertedWith("Ownable: caller is not the owner");
      
      await expect(
        dex.connect(trader).setPairFeeRate(pairId, 400)
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("Pausing", function () {
    beforeEach(async function () {
      const tx = await dex.createPair(tokenA.address, tokenB.address, 300);
      const receipt = await tx.wait();
      
      // Get pairId from event
      const pairCreatedEvent = receipt.events.find(e => e.event === "PairCreated");
      pairId = pairCreatedEvent.args.pairId;
    });

    it("Should allow owner to pause", async function () {
      await dex.pause();
      expect(await dex.paused()).to.be.true;
    });

    it("Should not allow operations when paused", async function () {
      await dex.pause();
      
      await expect(
        dex.connect(liquidityProvider).addLiquidity(
          pairId, 
          ethers.utils.parseEther("100"), 
          ethers.utils.parseEther("200"), 
          0, 
          0
        )
      ).to.be.revertedWith("Pausable: paused");
    });

    it("Should allow owner to unpause", async function () {
      await dex.pause();
      await dex.unpause();
      expect(await dex.paused()).to.be.false;
    });

    it("Should not allow non-owner to pause", async function () {
      await expect(
        dex.connect(trader).pause()
      ).to.be.revertedWith("Ownable: caller is not the owner");
    });
  });

  describe("View Functions", function () {
    beforeEach(async function () {
      const tx = await dex.createPair(tokenA.address, tokenB.address, 300);
      const receipt = await tx.wait();
      
      // Get pairId from event
      const pairCreatedEvent = receipt.events.find(e => e.event === "PairCreated");
      pairId = pairCreatedEvent.args.pairId;
      
      // Add liquidity for testing
      const amountA = ethers.utils.parseEther("100");
      const amountB = ethers.utils.parseEther("200");
      await dex.connect(liquidityProvider).addLiquidity(pairId, amountA, amountB, 0, 0);
    });

    it("Should get pair information", async function () {
      const pair = await dex.getPairInfo(pairId);
      
      expect(pair.active).to.be.true;
      expect(pair.reserveA).to.be.gt(0);
      expect(pair.reserveB).to.be.gt(0);
      expect(pair.totalLiquidity).to.be.gt(0);
      expect(pair.feeRate).to.equal(300);
    });

    it("Should get user liquidity position", async function () {
      const position = await dex.getUserPosition(pairId, liquidityProvider.address);
      
      expect(position.liquidity).to.be.gt(0);
      expect(position.lastDepositTime).to.be.gt(0);
    });

    it("Should return empty position for non-participant", async function () {
      const position = await dex.getUserPosition(pairId, trader.address);
      
      expect(position.liquidity).to.equal(0);
      expect(position.lastDepositTime).to.equal(0);
    });
  });
});