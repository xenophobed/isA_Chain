# isA_Chain Backend Services

Complete HTTP API adapter layer for isA_Chain smart contracts, following microservices architecture.

## 🏗️ Architecture

```
Gateway (8000) → Service Discovery (Consul) → Microservices → Smart Contracts
```

## 📦 Services Overview

| Service | Port | Status | Description |
|---------|------|--------|-------------|
| **defi-service** | 8311 | ✅ Ready | DeFi operations (swap, stake, lend, farm) |
| **nft-service** | 8312 | ✅ Ready | NFT minting, marketplace, collections |
| **tools-service** | 8315 | ✅ Ready | Developer tools, generic contract calls |
| oracle-service | 8313 | 📋 Planned | Price feeds and external data |
| privacy-service | 8314 | 📋 Planned | Privacy pool and mixer |
| exchange-service | 8316 | 📋 Planned | Order book and spot trading |

## 🚀 Quick Start

### Prerequisites

1. **Node.js 18+** and **npm**
2. **Blockchain node** running on `localhost:8545`
3. **Consul** (optional) running on `localhost:8500`
4. **Gateway** running on `localhost:8000`

### Start All Services

```bash
# Make the script executable
chmod +x start-all-services.sh

# Start all services
./start-all-services.sh

# Check status
./start-all-services.sh status

# View logs
./start-all-services.sh logs

# Stop all services
./start-all-services.sh stop
```

### Start Individual Service

```bash
# Navigate to service directory
cd defi-service

# Install dependencies
npm install

# Copy environment config
cp .env.example .env

# Start in development mode
npm run dev

# Or build and start in production
npm run build
npm start
```

## 🔗 Service Endpoints

### DeFi Service (8311)

**Swap & Liquidity**
- `GET  /api/v1/pools` - List all liquidity pools
- `POST /api/v1/swap/quote` - Get swap quote
- `POST /api/v1/swap/execute` - Execute swap
- `POST /api/v1/liquidity/add` - Add liquidity
- `POST /api/v1/liquidity/remove` - Remove liquidity

**Staking**
- `POST /api/v1/stake` - Stake tokens
- `POST /api/v1/unstake` - Unstake tokens
- `GET  /api/v1/stake/rewards/:address` - Get rewards
- `POST /api/v1/stake/claim` - Claim rewards

**Yield Farming**
- `GET  /api/v1/farms` - List all farms
- `POST /api/v1/farm/deposit` - Deposit to farm
- `POST /api/v1/farm/withdraw` - Withdraw from farm
- `POST /api/v1/farm/harvest` - Harvest yields

**Lending**
- `GET  /api/v1/lending/markets` - List lending markets
- `POST /api/v1/lending/supply` - Supply assets
- `POST /api/v1/lending/borrow` - Borrow assets
- `POST /api/v1/lending/repay` - Repay loan

### NFT Service (8312)

**Collections**
- `GET  /api/v1/collections` - List collections
- `POST /api/v1/collections/create` - Create collection
- `GET  /api/v1/collections/:address` - Collection details
- `GET  /api/v1/collections/:address/stats` - Collection statistics

**Minting**
- `POST /api/v1/mint` - Mint single NFT
- `POST /api/v1/mint/batch` - Batch mint NFTs
- `POST /api/v1/mint/lazy` - Lazy mint (gasless)

**NFT Management**
- `GET  /api/v1/tokens/:tokenId` - Token details
- `GET  /api/v1/owner/:address/tokens` - Tokens by owner
- `POST /api/v1/transfer` - Transfer NFT
- `POST /api/v1/burn` - Burn NFT

**Marketplace**
- `GET  /api/v1/marketplace/listings` - Active listings
- `POST /api/v1/marketplace/list` - List NFT for sale
- `POST /api/v1/marketplace/buy` - Buy NFT
- `POST /api/v1/marketplace/offer` - Make offer

**Auctions**
- `POST /api/v1/marketplace/auction/create` - Create auction
- `POST /api/v1/marketplace/auction/bid` - Place bid
- `POST /api/v1/marketplace/auction/end` - End auction

### Tools Service (8315)

**Contract Interaction**
- `POST /api/v1/contract/call` - Generic contract call
- `POST /api/v1/contract/read` - Read contract state
- `POST /api/v1/contract/write` - Write to contract
- `POST /api/v1/contract/deploy` - Deploy contract

**ABI Tools**
- `POST /api/v1/abi/register` - Register ABI
- `POST /api/v1/abi/encode` - Encode function call
- `POST /api/v1/abi/decode` - Decode function data
- `POST /api/v1/abi/decode-logs` - Decode event logs

**Transaction Tools**
- `POST /api/v1/transaction/estimate-gas` - Estimate gas
- `POST /api/v1/transaction/simulate` - Simulate transaction
- `GET  /api/v1/transaction/trace/:hash` - Trace transaction
- `POST /api/v1/transaction/send-raw` - Send raw transaction

**Event Filtering**
- `POST /api/v1/events/filter` - Filter past events
- `POST /api/v1/events/subscribe` - Subscribe to events

**Utilities**
- `POST /api/v1/utils/keccak256` - Hash data
- `POST /api/v1/signature/sign` - Sign message
- `POST /api/v1/signature/verify` - Verify signature
- `POST /api/v1/debug/revert-reason` - Get revert reason

## 🔧 Configuration

Each service uses environment variables for configuration. See `.env.example` in each service directory.

### Common Configuration

```env
# Service
PORT=8311
NODE_ENV=development

# Blockchain
BLOCKCHAIN_RPC_URL=http://localhost:8545
CHAIN_ID=1337
PRIVATE_KEY=0x...

# Consul
CONSUL_HOST=localhost
CONSUL_PORT=8500

# Gateway
GATEWAY_URL=http://localhost:8000
```

## 📡 Service Discovery

Services automatically register with Consul if available. This enables:
- Dynamic service discovery
- Health checking
- Configuration management
- Load balancing

View registered services:
```bash
# Consul UI
http://localhost:8500/ui

# CLI
consul catalog services
```

## 🧪 Testing

### Unit Tests
```bash
cd defi-service
npm test
```

### Integration Tests
```bash
# Test through Gateway
curl http://localhost:8000/api/v1/defi-service/pools

# Test directly
curl http://localhost:8311/health
```

### Load Testing
```bash
# Using Apache Bench
ab -n 1000 -c 10 http://localhost:8311/health

# Using k6
k6 run tests/load-test.js
```

## 🐳 Docker Support

### Build Images
```bash
# Build all services
docker-compose build

# Build specific service
docker build -t isa-chain/defi-service ./defi-service
```

### Run with Docker Compose
```bash
# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop all services
docker-compose down
```

## 📊 Monitoring

### Health Endpoints
Each service exposes a health endpoint:
```bash
curl http://localhost:8311/health
curl http://localhost:8312/health
curl http://localhost:8315/health
```

### Metrics
Services expose Prometheus metrics:
```bash
curl http://localhost:8311/metrics
```

### Logging
Services use structured JSON logging. Logs are written to:
- Console (development)
- `service.log` file
- Can be aggregated to ELK stack

## 🔒 Security

### Authentication
Services support JWT authentication via Gateway:
```http
Authorization: Bearer <jwt-token>
```

### Rate Limiting
- Default: 100 requests per 15 minutes per IP
- Tools service: 500 requests per 15 minutes

### Input Validation
All endpoints validate input using Joi schemas.

## 🚧 Development

### Adding New Service

1. Copy service template:
```bash
cp -r defi-service my-service
cd my-service
```

2. Update configuration:
- Edit `package.json` name
- Update port in `.env`
- Modify `src/index.ts`

3. Implement business logic:
- Add controllers in `src/controllers/`
- Add services in `src/services/`
- Add contract interfaces in `src/services/contracts/`

4. Register with start script:
- Add to `start-all-services.sh`

### Code Standards
- TypeScript for type safety
- ESLint for code quality
- Prettier for formatting
- Jest for testing

## 📚 Documentation

- [Architecture Guide](../../docs/architecture/hybrid-service-architecture.md)
- [Implementation Guide](../../docs/architecture/service-implementation-guide.md)
- [API Reference](../../docs/api-reference/api.md)

## 🤝 Contributing

1. Create feature branch
2. Make changes
3. Add tests
4. Submit PR

## 📝 License

Part of the isA_Chain ecosystem.

## 🆘 Troubleshooting

### Service won't start
- Check if port is already in use
- Verify Node.js version (18+)
- Check blockchain connection
- Review service logs

### Consul registration fails
- Ensure Consul is running
- Check network connectivity
- Verify service health endpoint

### Contract calls fail
- Verify contract addresses in `.env`
- Check blockchain RPC URL
- Ensure sufficient gas
- Review ABI compatibility

## 📞 Support

- GitHub Issues: [isA_Chain/issues](https://github.com/isa-chain/issues)
- Documentation: [docs.isa-chain.io](https://docs.isa-chain.io)