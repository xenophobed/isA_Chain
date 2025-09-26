const { ethers, upgrades } = require("hardhat");
const fs = require("fs");
const path = require("path");

async function main() {
    console.log("Starting deployment...");
    
    // Get signers
    const [deployer, treasury] = await ethers.getSigners();
    console.log("Deployer address:", deployer.address);
    console.log("Treasury address:", treasury.address);
    
    // Check deployer balance
    const deployerBalance = await ethers.provider.getBalance(deployer.address);
    console.log("Deployer balance:", ethers.formatEther(deployerBalance), "ETH");
    
    const deployments = {};
    
    try {
        // Deploy ISA Token
        console.log("\n📝 Deploying ISA Token...");
        const ISAToken = await ethers.getContractFactory("ISAToken");
        const isaToken = await ISAToken.deploy(treasury.address);
        await isaToken.waitForDeployment();
        
        const isaTokenAddress = await isaToken.getAddress();
        deployments.ISAToken = isaTokenAddress;
        console.log("✅ ISA Token deployed to:", isaTokenAddress);
        
        // Deploy Timelock Controller (needed for governance)
        console.log("\n⏰ Deploying Timelock Controller...");
        const TimelockController = await ethers.getContractFactory("TimelockController");
        const minDelay = 86400; // 1 day
        const proposers = [deployer.address]; // Will be updated after governor deployment
        const executors = [deployer.address]; // Will be updated after governor deployment
        const admin = deployer.address; // Will renounce after setup
        
        const timelock = await TimelockController.deploy(
            minDelay,
            proposers,
            executors,
            admin
        );
        await timelock.waitForDeployment();
        
        const timelockAddress = await timelock.getAddress();
        deployments.TimelockController = timelockAddress;
        console.log("✅ Timelock Controller deployed to:", timelockAddress);
        
        // Deploy ISA Governor
        console.log("\n🏛️ Deploying ISA Governor...");
        const ISAGovernor = await ethers.getContractFactory("ISAGovernor");
        const isaGovernor = await ISAGovernor.deploy(isaTokenAddress, timelockAddress);
        await isaGovernor.waitForDeployment();
        
        const isaGovernorAddress = await isaGovernor.getAddress();
        deployments.ISAGovernor = isaGovernorAddress;
        console.log("✅ ISA Governor deployed to:", isaGovernorAddress);
        
        // Deploy Simple DEX
        console.log("\n💱 Deploying Simple DEX...");
        const SimpleDEX = await ethers.getContractFactory("SimpleDEX");
        const simpleDEX = await SimpleDEX.deploy(treasury.address);
        await simpleDEX.waitForDeployment();
        
        const simpleDEXAddress = await simpleDEX.getAddress();
        deployments.SimpleDEX = simpleDEXAddress;
        console.log("✅ Simple DEX deployed to:", simpleDEXAddress);
        
        // Setup governance roles
        console.log("\n⚙️ Setting up governance roles...");
        
        // Grant timelock roles to governor
        const PROPOSER_ROLE = await timelock.PROPOSER_ROLE();
        const EXECUTOR_ROLE = await timelock.EXECUTOR_ROLE();
        const DEFAULT_ADMIN_ROLE = await timelock.DEFAULT_ADMIN_ROLE();
        
        await timelock.grantRole(PROPOSER_ROLE, isaGovernorAddress);
        await timelock.grantRole(EXECUTOR_ROLE, isaGovernorAddress);
        
        // Renounce admin role from deployer (governance only)
        await timelock.renounceRole(DEFAULT_ADMIN_ROLE, deployer.address);
        
        console.log("✅ Governance roles configured");
        
        // Setup DEX with ISA token support
        console.log("\n💱 Configuring DEX...");
        await simpleDEX.setSupportedToken(isaTokenAddress, true);
        console.log("✅ ISA token added to DEX");
        
        // Save deployment addresses
        const deploymentsPath = path.join(__dirname, "../deployments");
        if (!fs.existsSync(deploymentsPath)) {
            fs.mkdirSync(deploymentsPath, { recursive: true });
        }
        
        const network = hre.network.name;
        const deploymentFile = path.join(deploymentsPath, `${network}.json`);
        
        const deploymentData = {
            network: network,
            chainId: (await ethers.provider.getNetwork()).chainId.toString(),
            timestamp: new Date().toISOString(),
            deployer: deployer.address,
            treasury: treasury.address,
            contracts: deployments,
            verification: {
                ISAToken: {
                    address: deployments.ISAToken,
                    constructorArguments: [treasury.address]
                },
                TimelockController: {
                    address: deployments.TimelockController,
                    constructorArguments: [minDelay, proposers, executors, admin]
                },
                ISAGovernor: {
                    address: deployments.ISAGovernor,
                    constructorArguments: [isaTokenAddress, timelockAddress]
                },
                SimpleDEX: {
                    address: deployments.SimpleDEX,
                    constructorArguments: [treasury.address]
                }
            }
        };
        
        fs.writeFileSync(deploymentFile, JSON.stringify(deploymentData, null, 2));
        console.log(`\n📁 Deployment data saved to: ${deploymentFile}`);
        
        // Print deployment summary
        console.log("\n" + "=".repeat(60));
        console.log("🎉 DEPLOYMENT COMPLETE");
        console.log("=".repeat(60));
        console.log("Network:", network);
        console.log("Chain ID:", (await ethers.provider.getNetwork()).chainId.toString());
        console.log("Deployer:", deployer.address);
        console.log("Treasury:", treasury.address);
        console.log("\n📋 Contract Addresses:");
        
        Object.entries(deployments).forEach(([name, address]) => {
            console.log(`  ${name}: ${address}`);
        });
        
        console.log("\n🔍 Verification Commands:");
        Object.entries(deploymentData.verification).forEach(([name, data]) => {
            const args = data.constructorArguments.map(arg => `"${arg}"`).join(" ");
            console.log(`  npx hardhat verify --network ${network} ${data.address} ${args}`);
        });
        
        console.log("\n⚠️  Next Steps:");
        console.log("  1. Verify contracts on block explorer");
        console.log("  2. Create initial governance proposals");
        console.log("  3. Add more tokens to DEX");
        console.log("  4. Setup initial liquidity pools");
        console.log("  5. Configure additional DeFi protocols");
        
    } catch (error) {
        console.error("❌ Deployment failed:", error);
        
        // Save partial deployment data if any contracts were deployed
        if (Object.keys(deployments).length > 0) {
            const network = hre.network.name;
            const deploymentFile = path.join(__dirname, `../deployments/${network}-failed.json`);
            fs.writeFileSync(deploymentFile, JSON.stringify({
                network,
                timestamp: new Date().toISOString(),
                status: "FAILED",
                error: error.message,
                partialDeployments: deployments
            }, null, 2));
            console.log(`Partial deployment data saved to: ${deploymentFile}`);
        }
        
        throw error;
    }
}

// Error handling
main()
    .then(() => process.exit(0))
    .catch((error) => {
        console.error(error);
        process.exit(1);
    });