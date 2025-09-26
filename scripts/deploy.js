const { ethers } = require("hardhat");

async function main() {
  console.log("Deploying SimpleToken...");

  const [deployer] = await ethers.getSigners();
  console.log("Deploying contracts with account:", deployer.address);
  console.log("Account balance:", (await ethers.provider.getBalance(deployer.address)).toString());

  const SimpleToken = await ethers.getContractFactory("SimpleToken");
  const simpleToken = await SimpleToken.deploy();

  console.log("SimpleToken deployed to:", await simpleToken.getAddress());
  console.log("Transaction hash:", simpleToken.deploymentTransaction().hash);
  
  // Test basic functionality
  console.log("\nTesting basic functionality:");
  const totalSupply = await simpleToken.totalSupply();
  console.log("Total Supply:", ethers.formatEther(totalSupply), "SIMPLE");
  
  const ownerBalance = await simpleToken.balanceOf(deployer.address);
  console.log("Owner Balance:", ethers.formatEther(ownerBalance), "SIMPLE");
  
  console.log("\nDeployment completed successfully! ✅");
}

main()
  .then(() => process.exit(0))
  .catch((error) => {
    console.error(error);
    process.exit(1);
  });