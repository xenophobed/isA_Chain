export const config = {
    server: {
        port: parseInt(process.env.PORT || '8315'),
        host: process.env.HOST || '0.0.0.0'
    },
    blockchain: {
        rpcUrl: process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545',
        privateKey: process.env.PRIVATE_KEY || undefined,
        chainId: parseInt(process.env.CHAIN_ID || '1337')
    },
    cache: {
        enabled: process.env.CACHE_ENABLED !== 'false',
        ttl: parseInt(process.env.CACHE_TTL || '300'), // 5 minutes
        type: process.env.CACHE_TYPE || 'memory'
    },
    consul: {
        enabled: process.env.CONSUL_ENABLED === 'true',
        host: process.env.CONSUL_HOST || 'localhost',
        port: parseInt(process.env.CONSUL_PORT || '8500')
    },
    logging: {
        level: process.env.LOG_LEVEL || 'info',
        format: process.env.LOG_FORMAT || 'json'
    },
    api: {
        rateLimit: {
            windowMs: 15 * 60 * 1000, // 15 minutes
            max: parseInt(process.env.RATE_LIMIT_MAX || '500') // requests per windowMs
        },
        timeout: parseInt(process.env.API_TIMEOUT || '30000'), // 30 seconds
        maxPayload: process.env.MAX_PAYLOAD_SIZE || '10mb'
    },
    tools: {
        gasPrice: {
            multiplier: parseFloat(process.env.GAS_PRICE_MULTIPLIER || '1.2'),
            min: process.env.MIN_GAS_PRICE || '1000000000', // 1 gwei
            max: process.env.MAX_GAS_PRICE || '100000000000' // 100 gwei
        },
        simulation: {
            enabled: process.env.SIMULATION_ENABLED !== 'false',
            forkUrl: process.env.SIMULATION_FORK_URL || undefined
        }
    }
};