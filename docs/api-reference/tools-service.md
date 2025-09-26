# Tools Service API Reference

## Overview
The Tools Service provides developer tools for smart contract interactions including generic contract calls, ABI encoding/decoding, transaction debugging, gas estimation, event filtering, and blockchain queries.

**Base URL**: `http://localhost:8315`  
**Gateway URL**: `http://localhost:8000/api/v1/tools-service`

## Health Check

### GET /health
Check service health status.

**Request:**
```bash
curl -X GET http://localhost:8315/health
```

**Response:**
```json
{
  "status": "healthy",
  "service": "tools-service",
  "version": "1.0.0",
  "timestamp": "2025-09-25T02:31:05.778Z"
}
```

## Contract Interaction

### POST /api/v1/contract/call
Generic contract method call (read-only).

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/contract/call \
  -H "Content-Type: application/json" \
  -d '{
    "contract": "0x...",
    "abi": [...],
    "method": "balanceOf",
    "params": ["0x123..."]
  }'
```

**Response:**
```json
{
  "success": true,
  "result": "1000000000000000000"
}
```

### POST /api/v1/contract/read
Read contract state.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/contract/read \
  -H "Content-Type: application/json" \
  -d '{
    "contract": "0x...",
    "method": "totalSupply",
    "params": []
  }'
```

**Response:**
```json
{
  "success": true,
  "result": "1000000000000000000000"
}
```

### POST /api/v1/contract/write
Write to contract (transaction).

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/contract/write \
  -H "Content-Type: application/json" \
  -d '{
    "contract": "0x...",
    "abi": [...],
    "method": "transfer",
    "params": ["0x123...", "1000000000000000000"],
    "gasLimit": 100000,
    "value": "0"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "gasUsed": "21000"
}
```

### POST /api/v1/contract/deploy
Deploy a new contract.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/contract/deploy \
  -H "Content-Type: application/json" \
  -d '{
    "bytecode": "0x608060...",
    "abi": [...],
    "constructorArgs": ["My Token", "MTK", 18]
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x456...",
  "contractAddress": "0xabc...",
  "gasUsed": "500000"
}
```

## ABI Management

### POST /api/v1/abi/register
Register an ABI for a contract.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/abi/register \
  -H "Content-Type: application/json" \
  -d '{
    "contractAddress": "0x...",
    "abi": [
      {
        "inputs": [],
        "name": "totalSupply",
        "outputs": [{"type": "uint256"}],
        "type": "function"
      }
    ],
    "name": "MyContract"
  }'
```

**Response:**
```json
{
  "success": true,
  "result": "ABI registered"
}
```

### GET /api/v1/abi/:contractAddress
Get registered ABI for a contract.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/abi/0x123...
```

**Response:**
```json
{
  "success": true,
  "abi": [
    {
      "inputs": [],
      "name": "totalSupply",
      "outputs": [{"type": "uint256"}],
      "type": "function"
    }
  ]
}
```

### POST /api/v1/abi/encode
Encode function call data.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/abi/encode \
  -H "Content-Type: application/json" \
  -d '{
    "abi": [...],
    "functionName": "transfer",
    "params": ["0x123...", "1000000000000000000"]
  }'
```

**Response:**
```json
{
  "success": true,
  "encoded": "0xa9059cbb000000000000000000000000123...00000000000000000de0b6b3a7640000"
}
```

### POST /api/v1/abi/decode
Decode function call data.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/abi/decode \
  -H "Content-Type: application/json" \
  -d '{
    "abi": [...],
    "data": "0xa9059cbb000000..."
  }'
```

**Response:**
```json
{
  "success": true,
  "decoded": {
    "name": "transfer",
    "inputs": {
      "to": "0x123...",
      "amount": "1000000000000000000"
    }
  }
}
```

### POST /api/v1/abi/decode-logs
Decode event logs.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/abi/decode-logs \
  -H "Content-Type: application/json" \
  -d '{
    "abi": [...],
    "logs": [
      {
        "topics": ["0xddf252..."],
        "data": "0x00000..."
      }
    ]
  }'
```

**Response:**
```json
{
  "success": true,
  "decoded": [
    {
      "event": "Transfer",
      "args": {
        "from": "0x111...",
        "to": "0x222...",
        "value": "1000000000000000000"
      }
    }
  ]
}
```

## Transaction Tools

### POST /api/v1/transaction/estimate-gas
Estimate gas for a transaction.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/transaction/estimate-gas \
  -H "Content-Type: application/json" \
  -d '{
    "to": "0x...",
    "from": "0x...",
    "data": "0x...",
    "value": "0"
  }'
```

**Response:**
```json
{
  "success": true,
  "gasEstimate": "21000"
}
```

### POST /api/v1/transaction/simulate
Simulate a transaction without broadcasting.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/transaction/simulate \
  -H "Content-Type: application/json" \
  -d '{
    "to": "0x...",
    "from": "0x...",
    "data": "0x...",
    "value": "1000000000000000000"
  }'
```

**Response:**
```json
{
  "success": true,
  "simulation": "success",
  "returnData": "0x...",
  "logs": []
}
```

### GET /api/v1/transaction/trace/:hash
Get transaction trace.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/transaction/trace/0x123...
```

**Response:**
```json
{
  "success": true,
  "trace": [
    {
      "type": "CALL",
      "from": "0x111...",
      "to": "0x222...",
      "value": "0",
      "gas": "50000",
      "gasUsed": "21000",
      "input": "0x...",
      "output": "0x..."
    }
  ]
}
```

### POST /api/v1/transaction/send-raw
Send a signed raw transaction.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/transaction/send-raw \
  -H "Content-Type: application/json" \
  -d '{
    "signedTransaction": "0xf86c..."
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123..."
}
```

### GET /api/v1/transaction/receipt/:hash
Get transaction receipt.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/transaction/receipt/0x123...
```

**Response:**
```json
{
  "success": true,
  "receipt": {
    "transactionHash": "0x123...",
    "blockNumber": 12345,
    "gasUsed": "21000",
    "status": 1,
    "logs": []
  }
}
```

## Event Filtering

### POST /api/v1/events/filter
Filter blockchain events.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/events/filter \
  -H "Content-Type: application/json" \
  -d '{
    "address": "0x...",
    "topics": ["0xddf252..."],
    "fromBlock": 1000000,
    "toBlock": "latest"
  }'
```

**Response:**
```json
{
  "success": true,
  "events": [
    {
      "address": "0x...",
      "blockNumber": 1000001,
      "transactionHash": "0x...",
      "topics": ["0xddf252..."],
      "data": "0x...",
      "event": "Transfer",
      "args": {
        "from": "0x111...",
        "to": "0x222...",
        "value": "1000000000000000000"
      }
    }
  ]
}
```

### POST /api/v1/events/subscribe
Subscribe to events (WebSocket).

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/events/subscribe \
  -H "Content-Type: application/json" \
  -d '{
    "address": "0x...",
    "events": ["Transfer", "Approval"]
  }'
```

**Response:**
```json
{
  "success": true,
  "subscriptionId": "sub_123"
}
```

### DELETE /api/v1/events/unsubscribe/:subscriptionId
Unsubscribe from events.

**Request:**
```bash
curl -X DELETE http://localhost:8315/api/v1/events/unsubscribe/sub_123
```

**Response:**
```json
{
  "success": true,
  "result": "unsubscribed"
}
```

## Blockchain Queries

### GET /api/v1/block/:blockNumber
Get block information.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/block/12345
```

**Response:**
```json
{
  "success": true,
  "block": {
    "number": 12345,
    "hash": "0xabc...",
    "parentHash": "0xdef...",
    "timestamp": 1234567890,
    "gasLimit": "15000000",
    "gasUsed": "5000000",
    "transactions": ["0x123...", "0x456..."]
  }
}
```

### GET /api/v1/account/:address
Get account information.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/account/0x123...
```

**Response:**
```json
{
  "success": true,
  "account": {
    "address": "0x123...",
    "balance": "1000000000000000000",
    "nonce": 5,
    "code": "0x"
  }
}
```

### GET /api/v1/code/:address
Get contract bytecode.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/code/0xabc...
```

**Response:**
```json
{
  "success": true,
  "code": "0x608060..."
}
```

### GET /api/v1/storage/:address/:slot
Get storage at slot.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/storage/0xabc.../0
```

**Response:**
```json
{
  "success": true,
  "storage": "0x0000000000000000000000000000000000000000000000000000000000000064"
}
```

### GET /api/v1/nonce/:address
Get account nonce.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/nonce/0x123...
```

**Response:**
```json
{
  "success": true,
  "nonce": 5
}
```

### GET /api/v1/gas-price
Get current gas price.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/gas-price
```

**Response:**
```json
{
  "success": true,
  "gasPrice": "20000000000",
  "gasPriceGwei": "20"
}
```

## Signature Tools

### POST /api/v1/signature/sign
Sign a message.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/signature/sign \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Hello, World!",
    "privateKey": "0x..."
  }'
```

**Response:**
```json
{
  "success": true,
  "signature": "0x..."
}
```

### POST /api/v1/signature/verify
Verify a signature.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/signature/verify \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Hello, World!",
    "signature": "0x...",
    "address": "0x123..."
  }'
```

**Response:**
```json
{
  "success": true,
  "valid": true
}
```

### POST /api/v1/signature/recover
Recover address from signature.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/signature/recover \
  -H "Content-Type: application/json" \
  -d '{
    "message": "Hello, World!",
    "signature": "0x..."
  }'
```

**Response:**
```json
{
  "success": true,
  "address": "0x123..."
}
```

## Utility Functions

### POST /api/v1/utils/keccak256
Calculate Keccak256 hash.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/utils/keccak256 \
  -H "Content-Type: application/json" \
  -d '{
    "data": "Hello, World!"
  }'
```

**Response:**
```json
{
  "success": true,
  "hash": "0xacaf3289d7b601cbd114fb36c4d29c85bbfd5e133f14cb355c3fd8d99367964f"
}
```

### POST /api/v1/utils/encode-packed
Encode packed data.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/utils/encode-packed \
  -H "Content-Type: application/json" \
  -d '{
    "types": ["address", "uint256"],
    "values": ["0x123...", "1000"]
  }'
```

**Response:**
```json
{
  "success": true,
  "encoded": "0x..."
}
```

### POST /api/v1/utils/to-hex
Convert to hexadecimal.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/utils/to-hex \
  -H "Content-Type: application/json" \
  -d '{
    "value": "12345"
  }'
```

**Response:**
```json
{
  "success": true,
  "hex": "0x3039"
}
```

### POST /api/v1/utils/from-hex
Convert from hexadecimal.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/utils/from-hex \
  -H "Content-Type: application/json" \
  -d '{
    "hex": "0x3039"
  }'
```

**Response:**
```json
{
  "success": true,
  "decoded": "12345"
}
```

### POST /api/v1/utils/checksum-address
Get checksum address.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/utils/checksum-address \
  -H "Content-Type: application/json" \
  -d '{
    "address": "0xabc..."
  }'
```

**Response:**
```json
{
  "success": true,
  "address": "0xAbC..."
}
```

## Contract Verification

### POST /api/v1/verify/contract
Verify contract source code.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/verify/contract \
  -H "Content-Type: application/json" \
  -d '{
    "address": "0x...",
    "sourceCode": "pragma solidity ^0.8.0;...",
    "contractName": "MyToken",
    "compilerVersion": "v0.8.19",
    "optimization": true,
    "runs": 200
  }'
```

**Response:**
```json
{
  "success": true,
  "guid": "verify_123"
}
```

### GET /api/v1/verify/status/:guid
Check verification status.

**Request:**
```bash
curl -X GET http://localhost:8315/api/v1/verify/status/verify_123
```

**Response:**
```json
{
  "success": true,
  "status": "verified",
  "message": "Contract verified successfully"
}
```

## Debugging Tools

### POST /api/v1/debug/call-trace
Debug a contract call.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/debug/call-trace \
  -H "Content-Type: application/json" \
  -d '{
    "to": "0x...",
    "data": "0x...",
    "from": "0x...",
    "value": "0"
  }'
```

**Response:**
```json
{
  "success": true,
  "trace": [
    {
      "pc": 0,
      "op": "PUSH1",
      "gas": 50000,
      "gasCost": 3,
      "depth": 1,
      "stack": []
    }
  ]
}
```

### POST /api/v1/debug/storage-diff
Get storage differences.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/debug/storage-diff \
  -H "Content-Type: application/json" \
  -d '{
    "address": "0x...",
    "fromBlock": 1000,
    "toBlock": 2000
  }'
```

**Response:**
```json
{
  "success": true,
  "diff": [
    {
      "slot": "0x0",
      "before": "0x64",
      "after": "0xc8"
    }
  ]
}
```

### POST /api/v1/debug/revert-reason
Get revert reason for failed transaction.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/debug/revert-reason \
  -H "Content-Type: application/json" \
  -d '{
    "txHash": "0x..."
  }'
```

**Response:**
```json
{
  "success": true,
  "reason": "Insufficient balance"
}
```

## Batch Operations

### POST /api/v1/batch/call
Batch multiple calls.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/batch/call \
  -H "Content-Type: application/json" \
  -d '{
    "calls": [
      {
        "to": "0x...",
        "data": "0x..."
      },
      {
        "to": "0x...",
        "data": "0x..."
      }
    ]
  }'
```

**Response:**
```json
{
  "success": true,
  "results": [
    "0x...",
    "0x..."
  ]
}
```

### POST /api/v1/batch/transaction
Send batch transactions.

**Request:**
```bash
curl -X POST http://localhost:8315/api/v1/batch/transaction \
  -H "Content-Type: application/json" \
  -d '{
    "transactions": [
      {
        "to": "0x...",
        "value": "1000000000000000000",
        "data": "0x"
      }
    ]
  }'
```

**Response:**
```json
{
  "success": true,
  "results": [
    {
      "txHash": "0x...",
      "status": "success"
    }
  ]
}
```

## WebSocket Support

The service supports WebSocket connections for real-time event subscriptions:

```javascript
const ws = new WebSocket('ws://localhost:8315');

ws.send(JSON.stringify({
  type: 'subscribe',
  address: '0x...',
  events: ['Transfer']
}));

ws.on('message', (data) => {
  console.log('Event received:', JSON.parse(data));
});
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
- **Limit**: 500 requests per 15 minutes per IP (more lenient for developer tools)
- **Headers**: Rate limit information is included in response headers

## Notes

- All values are in wei unless otherwise specified
- Addresses must be valid Ethereum addresses
- Private keys should never be sent to production APIs
- The service caches frequently accessed data for performance
- WebSocket connections are available for real-time event monitoring
- Gas estimates may vary based on network conditions