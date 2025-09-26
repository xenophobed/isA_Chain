export class ABIService {
    private cacheService: any;

    constructor(cacheService?: any) {
        this.cacheService = cacheService;
    }

    async getABI(contractAddress: string) {
        return {
            success: true,
            abi: [],
            address: contractAddress
        };
    }

    async decodeTransaction(txData: string) {
        return {
            success: true,
            decoded: {
                function: 'unknown',
                parameters: []
            }
        };
    }

    async encodeFunction(abi: any[], functionName: string, params: any[]) {
        return {
            success: true,
            encoded: '0x00000000'
        };
    }

    async decodeFunction(abi: any[], data: string) {
        return {
            success: true,
            decoded: {
                name: 'unknown',
                inputs: []
            }
        };
    }

    async decodeLogs(abi: any[], logs: any[]) {
        return {
            success: true,
            decoded: []
        };
    }
}