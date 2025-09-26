# DeFi Service

HTTP API adapter for isA_Chain DeFi smart contracts.

## Architecture

This service provides RESTful APIs for interacting with DeFi smart contracts:
- **SimpleDEX**: Token swaps and liquidity pools
- **StakingPool**: Token staking and rewards
- **YieldFarming**: Yield farming operations
- **LendingProtocol**: Lending and borrowing

## Quick Start

### 1. Install dependencies
```bash
npm install
```

### 2. Configure environment
```bash
cp .env.example .env
# Edit .env with your contract addresses and RPC endpoint
```

### 3. Deploy smart contracts (if not already deployed)
```bash
cd ../../../  # Go to isA_Chain root
npx hardhat run scripts/deploy.js --network localhost
# Copy deployed contract addresses to .env
```

### 4. Start the service
```bash
npm run dev
```

The service will:
- Start on port 8311
- Register with Consul for service discovery
- Be accessible via Gateway at: http://localhost:8000/api/v1/defi-service/

## API Endpoints

### DEX Operations
- `GET  /api/v1/pools` - Get all liquidity pools
- `POST /api/v1/swap/quote` - Get swap quote
- `POST /api/v1/swap/execute` - Execute token swap
- `POST /api/v1/liquidity/add` - Add liquidity to pool
- `POST /api/v1/liquidity/remove` - Remove liquidity
- `GET  /api/v1/liquidity/positions/:address` - Get user's LP positions

### Staking Operations
- `POST /api/v1/stake` - Stake tokens
- `POST /api/v1/unstake` - Unstake tokens
- `GET  /api/v1/stake/rewards/:address` - Get pending rewards
- `POST /api/v1/stake/claim` - Claim staking rewards

### Yield Farming
- `GET  /api/v1/farms` - Get all farming pools
- `POST /api/v1/farm/deposit` - Deposit to farm
- `POST /api/v1/farm/withdraw` - Withdraw from farm
- `POST /api/v1/farm/harvest` - Harvest yield

### Lending Protocol
- `GET  /api/v1/lending/markets` - Get lending markets
- `POST /api/v1/lending/supply` - Supply assets
- `POST /api/v1/lending/borrow` - Borrow assets
- `POST /api/v1/lending/repay` - Repay loan
- `GET  /api/v1/lending/position/:address` - Get user position

## Testing via Gateway

```bash
# Get all pools
curl http://localhost:8000/api/v1/defi-service/pools

# Get swap quote
curl -X POST http://localhost:8000/api/v1/defi-service/swap/quote \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0x...",
    "tokenOut": "0x...",
    "amountIn": "1.0"
  }'

# Execute swap
curl -X POST http://localhost:8000/api/v1/defi-service/swap/execute \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0x...",
    "tokenOut": "0x...",
    "amountIn": "1.0",
    "minAmountOut": "0.95",
    "userAddress": "0x..."
  }'
```

## Development

### Project Structure
```
defi-service/
├── src/
│   ├── index.ts                 # Main server
│   ├── config.ts                # Configuration
│   ├── controllers/             # Request handlers
│   │   └── defi.controller.ts
│   ├── services/                # Business logic
│   │   ├── blockchain.service.ts
│   │   └── contracts/          # Contract-specific services
│   │       ├── simpleDEX.service.ts
│   │       ├── stakingPool.service.ts
│   │       ├── yieldFarming.service.ts
│   │       └── lendingProtocol.service.ts
│   ├── utils/                   # Utilities
│   └── abi/                     # Contract ABIs
```

### Adding New Contract Functions

1. Add the contract ABI to `src/abi/`
2. Create a service in `src/services/contracts/`
3. Add endpoints in the controller
4. Register routes in `src/index.ts`

## Integration with isA_Cloud Gateway

The service automatically registers with Consul, making it discoverable by the Gateway.

Access pattern:
```
Client → Gateway (8000) → Dynamic Proxy → DeFi Service (8311) → Smart Contracts
```

## Next Steps

1. Create similar services for:
   - NFT operations (`nft-service`)
   - Oracle data (`oracle-service`)
   - Privacy features (`privacy-service`)
   - Exchange operations (`exchange-service`)

2. Create a tools service for developers:
   - Generic contract calls
   - ABI encoding/decoding
   - Transaction debugging

## License

Part of the isA_Chain ecosystem.