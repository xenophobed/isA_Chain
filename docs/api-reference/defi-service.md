# DeFi Service API Reference

## Overview
The DeFi Service provides HTTP APIs for interacting with decentralized finance smart contracts including token swaps, liquidity management, staking, yield farming, and lending protocols.

**Base URL**: `http://localhost:8311`  
**Gateway URL**: `http://localhost:8000/api/v1/defi-service`

## Health Check

### GET /health
Check service health status.

**Request:**
```bash
curl -X GET http://localhost:8311/health
```

**Response:**
```json
{
  "status": "healthy",
  "service": "defi-service",
  "version": "1.0.0",
  "timestamp": "2025-09-25T02:28:28.637Z"
}
```

## Swap Endpoints

### GET /api/v1/pools
Get all available liquidity pools.

**Request:**
```bash
curl -X GET http://localhost:8311/api/v1/pools
```

**Response:**
```json
{
  "success": true,
  "pools": [
    {
      "id": "1",
      "token0": "0x...",
      "token1": "0x...",
      "reserve0": "1000000",
      "reserve1": "2000000",
      "totalSupply": "100000"
    }
  ]
}
```

### POST /api/v1/swap/quote
Get swap quote for token exchange.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/swap/quote \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0x...",
    "tokenOut": "0x...",
    "amountIn": "1000000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "quote": {
    "amountOut": "1980000000000000000",
    "priceImpact": "0.5",
    "fee": "3000000000000000"
  }
}
```

### POST /api/v1/swap/execute
Execute a token swap.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/swap/execute \
  -H "Content-Type: application/json" \
  -d '{
    "tokenIn": "0x...",
    "tokenOut": "0x...",
    "amountIn": "1000000000000000000",
    "amountOutMin": "1950000000000000000",
    "recipient": "0x...",
    "deadline": 1234567890
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "amountIn": "1000000000000000000",
  "amountOut": "1980000000000000000"
}
```

## Liquidity Endpoints

### POST /api/v1/liquidity/add
Add liquidity to a pool.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/liquidity/add \
  -H "Content-Type: application/json" \
  -d '{
    "token0": "0x...",
    "token1": "0x...",
    "amount0Desired": "1000000000000000000",
    "amount1Desired": "2000000000000000000",
    "amount0Min": "950000000000000000",
    "amount1Min": "1900000000000000000",
    "recipient": "0x...",
    "deadline": 1234567890
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x456...",
  "liquidity": "100000000000000000",
  "amount0": "1000000000000000000",
  "amount1": "2000000000000000000"
}
```

### POST /api/v1/liquidity/remove
Remove liquidity from a pool.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/liquidity/remove \
  -H "Content-Type: application/json" \
  -d '{
    "poolId": "1",
    "liquidity": "50000000000000000",
    "amount0Min": "450000000000000000",
    "amount1Min": "900000000000000000",
    "recipient": "0x...",
    "deadline": 1234567890
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x789...",
  "amount0": "500000000000000000",
  "amount1": "1000000000000000000"
}
```

### GET /api/v1/liquidity/positions/:address
Get liquidity positions for an address.

**Request:**
```bash
curl -X GET http://localhost:8311/api/v1/liquidity/positions/0x123...
```

**Response:**
```json
{
  "success": true,
  "positions": [
    {
      "poolId": "1",
      "liquidity": "100000000000000000",
      "token0": "0x...",
      "token1": "0x...",
      "value": {
        "token0": "1000000000000000000",
        "token1": "2000000000000000000"
      }
    }
  ]
}
```

## Staking Endpoints

### POST /api/v1/stake
Stake tokens.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/stake \
  -H "Content-Type: application/json" \
  -d '{
    "token": "0x...",
    "amount": "1000000000000000000",
    "duration": 86400
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0xabc...",
  "stakeId": "123",
  "apr": "12.5"
}
```

### POST /api/v1/unstake
Unstake tokens.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/unstake \
  -H "Content-Type: application/json" \
  -d '{
    "stakeId": "123"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0xdef...",
  "amount": "1000000000000000000",
  "rewards": "125000000000000000"
}
```

### GET /api/v1/stake/rewards/:address
Get staking rewards for an address.

**Request:**
```bash
curl -X GET http://localhost:8311/api/v1/stake/rewards/0x123...
```

**Response:**
```json
{
  "success": true,
  "rewards": {
    "earned": "125000000000000000",
    "unclaimed": "50000000000000000",
    "totalStaked": "1000000000000000000"
  }
}
```

### POST /api/v1/stake/claim
Claim staking rewards.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/stake/claim \
  -H "Content-Type: application/json" \
  -d '{
    "stakeId": "123"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "amount": "50000000000000000"
}
```

## Yield Farming Endpoints

### GET /api/v1/farms
Get all available yield farms.

**Request:**
```bash
curl -X GET http://localhost:8311/api/v1/farms
```

**Response:**
```json
{
  "success": true,
  "farms": [
    {
      "id": "1",
      "name": "ETH-USDT Farm",
      "lpToken": "0x...",
      "rewardToken": "0x...",
      "apr": "45.2",
      "tvl": "1000000"
    }
  ]
}
```

### POST /api/v1/farm/deposit
Deposit LP tokens to farm.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/farm/deposit \
  -H "Content-Type: application/json" \
  -d '{
    "farmId": "1",
    "amount": "1000000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "deposited": "1000000000000000000"
}
```

### POST /api/v1/farm/withdraw
Withdraw LP tokens from farm.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/farm/withdraw \
  -H "Content-Type: application/json" \
  -d '{
    "farmId": "1",
    "amount": "500000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x456...",
  "withdrawn": "500000000000000000"
}
```

### POST /api/v1/farm/harvest
Harvest yield farming rewards.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/farm/harvest \
  -H "Content-Type: application/json" \
  -d '{
    "farmId": "1"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x789...",
  "rewards": "250000000000000000"
}
```

## Lending Endpoints

### GET /api/v1/lending/markets
Get all lending markets.

**Request:**
```bash
curl -X GET http://localhost:8311/api/v1/lending/markets
```

**Response:**
```json
{
  "success": true,
  "markets": [
    {
      "asset": "0x...",
      "symbol": "ETH",
      "supplyAPY": "2.5",
      "borrowAPY": "4.2",
      "totalSupply": "1000000000000000000000",
      "totalBorrow": "500000000000000000000",
      "utilization": "50"
    }
  ]
}
```

### POST /api/v1/lending/supply
Supply assets to lending pool.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/lending/supply \
  -H "Content-Type: application/json" \
  -d '{
    "asset": "0x...",
    "amount": "1000000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "supplied": "1000000000000000000",
  "aTokenReceived": "1000000000000000000"
}
```

### POST /api/v1/lending/borrow
Borrow assets from lending pool.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/lending/borrow \
  -H "Content-Type: application/json" \
  -d '{
    "asset": "0x...",
    "amount": "500000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x456...",
  "borrowed": "500000000000000000",
  "interestRate": "4.2"
}
```

### POST /api/v1/lending/repay
Repay borrowed assets.

**Request:**
```bash
curl -X POST http://localhost:8311/api/v1/lending/repay \
  -H "Content-Type: application/json" \
  -d '{
    "asset": "0x...",
    "amount": "500000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x789...",
  "repaid": "500000000000000000",
  "remainingDebt": "0"
}
```

### GET /api/v1/lending/position/:address
Get lending position for an address.

**Request:**
```bash
curl -X GET http://localhost:8311/api/v1/lending/position/0x123...
```

**Response:**
```json
{
  "success": true,
  "position": {
    "supplied": [
      {
        "asset": "0x...",
        "amount": "1000000000000000000",
        "apy": "2.5"
      }
    ],
    "borrowed": [
      {
        "asset": "0x...",
        "amount": "500000000000000000",
        "apy": "4.2"
      }
    ],
    "healthFactor": "1.8"
  }
}
```

## Error Responses

All endpoints may return error responses in the following format:

```json
{
  "success": false,
  "error": "Error message description"
}
```

Common HTTP status codes:
- `200 OK` - Request successful
- `400 Bad Request` - Invalid request parameters
- `404 Not Found` - Resource not found
- `500 Internal Server Error` - Server error

## Rate Limiting

The API implements rate limiting:
- **Limit**: 100 requests per 15 minutes per IP
- **Headers**: Rate limit information is included in response headers

## Notes

- All amounts are in the smallest unit of the token (e.g., wei for ETH)
- Timestamps are Unix timestamps in seconds
- Addresses must be valid Ethereum addresses
- The service requires connection to a blockchain node (default: http://localhost:8545)