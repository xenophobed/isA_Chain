import { ethers } from 'ethers';
import isaNFTABI from '../../../../../contracts/abi/ISANFT.json';

export class IsaNFTService {
    private provider: ethers.JsonRpcProvider;
    private contract: ethers.Contract;
    private signer: ethers.Signer;

    constructor() {
        this.provider = new ethers.JsonRpcProvider(
            process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545'
        );
        
        const contractAddress = process.env.ISA_NFT_CONTRACT_ADDRESS || 
                              '0x0000000000000000000000000000000000000003';
        
        // Handle both direct ABI array and wrapped ABI object
        const abi = Array.isArray(isaNFTABI) ? isaNFTABI : (isaNFTABI.abi || isaNFTABI);
        
        this.contract = new ethers.Contract(
            contractAddress,
            abi,
            this.provider
        );

        // Setup signer if private key is provided
        if (process.env.PRIVATE_KEY) {
            this.signer = new ethers.Wallet(process.env.PRIVATE_KEY, this.provider);
            this.contract = this.contract.connect(this.signer);
        } else {
            // Use default signer for development
            this.provider.getSigner().then(signer => {
                this.signer = signer;
                this.contract = this.contract.connect(this.signer);
            });
        }
    }

    // Minting functions
    async mintSingle(params: {
        to: string;
        tokenURI: string;
        royaltyReceiver?: string;
        royaltyFeeNumerator?: number;
    }) {
        try {
            const tx = await this.contract.mintWithRoyalty(
                params.to,
                params.tokenURI,
                params.royaltyReceiver || params.to,
                params.royaltyFeeNumerator || 250 // 2.5% default
            );
            
            const receipt = await tx.wait();
            
            // Extract token ID from Transfer event
            const transferEvent = receipt.events?.find(
                (e: any) => e.event === 'Transfer'
            );
            const tokenId = transferEvent?.args?.tokenId;
            
            return {
                success: true,
                transactionHash: tx.hash,
                tokenId: tokenId?.toString(),
                gasUsed: receipt.gasUsed.toString()
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    async mintBatch(params: {
        to: string;
        tokenURIs: string[];
        royaltyReceiver?: string;
        royaltyFeeNumerator?: number;
    }) {
        try {
            const results = [];
            for (const tokenURI of params.tokenURIs) {
                const result = await this.mintSingle({
                    to: params.to,
                    tokenURI,
                    royaltyReceiver: params.royaltyReceiver,
                    royaltyFeeNumerator: params.royaltyFeeNumerator
                });
                results.push(result);
            }
            
            return {
                success: true,
                results
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    // Token management
    async getTokenDetails(tokenId: string) {
        try {
            const [owner, tokenURI, royaltyInfo] = await Promise.all([
                this.contract.ownerOf(tokenId),
                this.contract.tokenURI(tokenId),
                this.contract.royaltyInfo(tokenId, ethers.utils.parseEther("1"))
            ]);
            
            return {
                tokenId,
                owner,
                tokenURI,
                royaltyReceiver: royaltyInfo[0],
                royaltyAmount: ethers.utils.formatEther(royaltyInfo[1])
            };
        } catch (error: any) {
            throw new Error(`Failed to get token details: ${error.message}`);
        }
    }

    async getTokensByOwner(address: string) {
        try {
            const balance = await this.contract.balanceOf(address);
            const tokens = [];
            
            for (let i = 0; i < balance.toNumber(); i++) {
                const tokenId = await this.contract.tokenOfOwnerByIndex(address, i);
                const tokenURI = await this.contract.tokenURI(tokenId);
                tokens.push({
                    tokenId: tokenId.toString(),
                    tokenURI
                });
            }
            
            return tokens;
        } catch (error: any) {
            throw new Error(`Failed to get tokens by owner: ${error.message}`);
        }
    }

    async transferToken(params: {
        from: string;
        to: string;
        tokenId: string;
    }) {
        try {
            const tx = await this.contract.transferFrom(
                params.from,
                params.to,
                params.tokenId
            );
            
            const receipt = await tx.wait();
            
            return {
                success: true,
                transactionHash: tx.hash,
                gasUsed: receipt.gasUsed.toString()
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    async burnToken(tokenId: string) {
        try {
            const tx = await this.contract.burn(tokenId);
            const receipt = await tx.wait();
            
            return {
                success: true,
                transactionHash: tx.hash,
                gasUsed: receipt.gasUsed.toString()
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    // Collection management
    async getCollectionStats() {
        try {
            const [name, symbol, totalSupply] = await Promise.all([
                this.contract.name(),
                this.contract.symbol(),
                this.contract.totalSupply()
            ]);
            
            return {
                name,
                symbol,
                totalSupply: totalSupply.toString(),
                contractAddress: this.contract.address
            };
        } catch (error: any) {
            throw new Error(`Failed to get collection stats: ${error.message}`);
        }
    }

    // Royalty management
    async updateRoyalty(params: {
        tokenId: string;
        receiver: string;
        feeNumerator: number;
    }) {
        try {
            const tx = await this.contract.setTokenRoyalty(
                params.tokenId,
                params.receiver,
                params.feeNumerator
            );
            
            const receipt = await tx.wait();
            
            return {
                success: true,
                transactionHash: tx.hash,
                gasUsed: receipt.gasUsed.toString()
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    // Approval management
    async approve(params: {
        to: string;
        tokenId: string;
    }) {
        try {
            const tx = await this.contract.approve(params.to, params.tokenId);
            const receipt = await tx.wait();
            
            return {
                success: true,
                transactionHash: tx.hash,
                gasUsed: receipt.gasUsed.toString()
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    async setApprovalForAll(params: {
        operator: string;
        approved: boolean;
    }) {
        try {
            const tx = await this.contract.setApprovalForAll(
                params.operator,
                params.approved
            );
            const receipt = await tx.wait();
            
            return {
                success: true,
                transactionHash: tx.hash,
                gasUsed: receipt.gasUsed.toString()
            };
        } catch (error: any) {
            return {
                success: false,
                error: error.message
            };
        }
    }

    // Additional methods required by NFTController (stub implementations)
    async getAllCollections() {
        return { success: true, collections: [] };
    }

    async deployCollection(params: any) {
        return { success: true, address: '0x0000000000000000000000000000000000000000' };
    }

    async getCollectionInfo(address: string) {
        return this.getCollectionStats();
    }

    async mintNFT(params: any) {
        return this.mintSingle(params);
    }

    async createLazyMintVoucher(params: any) {
        return { success: true, voucher: {} };
    }

    async getTokenInfo(tokenId: string) {
        return this.getTokenDetails(tokenId);
    }

    async getTokenURI(tokenId: string) {
        const details = await this.getTokenDetails(tokenId);
        return details.tokenURI;
    }

    async getTokenTransferHistory(tokenId: string) {
        return { success: true, transfers: [] };
    }

    async transferNFT(params: any) {
        return this.transferToken(params);
    }

    async burnNFT(tokenId: string) {
        return this.burnToken(tokenId);
    }

    async updateTokenURI(params: any) {
        return { success: true };
    }

    async setRoyalty(params: any) {
        return this.updateRoyalty(params);
    }

    async getRoyaltyInfo(params: any) {
        return { success: true, royaltyInfo: {} };
    }

    async claimRoyalties(params: any) {
        return { success: true };
    }
}