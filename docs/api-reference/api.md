# isA_Chain Blockchain API Reference

## Overview

isA_Chain Gateway provides complete blockchain HTTP API supporting chain status queries, account balances, transaction management, and block information. All APIs are accessed through the unified gateway.

**Base URL**: `http://localhost:8000`  
**API Version**: v1  
**Authentication**: Some endpoints require authentication

## Authentication

Some API endpoints require authentication through middleware:

```http
Authorization: Bearer <your-jwt-token>
```

## Endpoints Overview

| Endpoint | Method | Description | Auth Required |
|----------|--------|-------------|---------------|
| `/health` | GET | Gateway health check | No |
| `/api/v1/blockchain/status` | GET | Get blockchain status | Yes |
| `/api/v1/blockchain/balance/{address}` | GET | Query address balance | Yes |
| `/api/v1/blockchain/transaction` | POST | Send transaction | Yes |
| `/api/v1/blockchain/transaction/{hash}` | GET | Query transaction details | Yes |
| `/api/v1/blockchain/block/{number}` | GET | Get block information | Yes |

---

## API Details

### 1. Gateway Health Check

Get Gateway service health status.

**Endpoint**: `GET /health`

#### Request Example
```bash
curl -X GET http://localhost:8000/health
```

#### Response Example
```json
{
  "service": "isa-cloud-gateway",
  "status": "healthy",
  "timestamp": "2025-09-22T14:01:07.716105Z",
  "version": "1.0.0"
}
```

#### Response Fields
| Field | Type | Description |
|-------|------|-------------|
| `service` | string | Service name |
| `status` | string | Health status |
| `timestamp` | string | Response timestamp (ISO 8601) |
| `version` | string | Service version |

---

### 2. Get Blockchain Status

Get current blockchain connection status and basic information.

**Endpoint**: `GET /api/v1/blockchain/status`

#### Request Example
```bash
curl -X GET http://localhost:8000/api/v1/blockchain/status \
  -H "Authorization: Bearer <your-jwt-token>"
```

#### Response Example
```json
{
  "block_number": 1000000,
  "chain_id": "1337",
  "chain_type": "isa_chain",
  "connected": true,
  "timestamp": 1758549713
}
```

#### Response Fields
| Field | Type | Description |
|-------|------|-------------|
| `block_number` | number | Current block height |
| `chain_id` | string | Chain ID |
| `chain_type` | string | Chain type (isa_chain) |
| `connected` | boolean | Connection status |
| `timestamp` | number | Unix timestamp |

---

### 3. Query Address Balance

Query ISA token balance for a specified address.

**Endpoint**: `GET /api/v1/blockchain/balance/{address}`

#### Path Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `address` | string | Yes | Wallet address (0x format) |

#### Request Example
```bash
curl -X GET http://localhost:8000/api/v1/blockchain/balance/0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5 \
  -H "Authorization: Bearer <your-jwt-token>"
```

#### Response Example
```json
{
  "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5",
  "balance": "1000000000000000000",
  "eth": "1",
  "wei": "1000000000000000000"
}
```

#### Response Fields
| Field | Type | Description |
|-------|------|-------------|
| `address` | string | Queried wallet address |
| `balance` | string | Balance (Wei unit) |
| `eth` | string | Balance (ETH unit) |
| `wei` | string | Balance (Wei unit) |

---

### 4. Send Transaction

Send a new transaction to the blockchain.

**Endpoint**: `POST /api/v1/blockchain/transaction`

#### Request Body
```json
{
  "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5",
  "value": "1000000000000000000",
  "data": "0x",
  "gasLimit": 21000,
  "gasPrice": "20000000000"
}
```

#### Request Fields
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `to` | string | Yes | Recipient address |
| `value` | string | No | Transfer amount (Wei) |
| `data` | string | No | Transaction data |
| `gasLimit` | number | No | Gas limit (default: 21000) |
| `gasPrice` | string | No | Gas price (default: 20 Gwei) |

#### Request Example
```bash
curl -X POST http://localhost:8000/api/v1/blockchain/transaction \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer <your-jwt-token>" \
  -d '{
    "to": "0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5",
    "value": "1000000000000000000",
    "gasLimit": 21000
  }'
```

#### Response Example
```json
{
  "status": "pending",
  "transaction_hash": "0xisa1758551005"
}
```

#### Response Fields
| Field | Type | Description |
|-------|------|-------------|
| `status` | string | Transaction status |
| `transaction_hash` | string | Transaction hash |

---

### 5. Query Transaction Details

Query detailed transaction information by transaction hash.

**Endpoint**: `GET /api/v1/blockchain/transaction/{hash}`

#### Path Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `hash` | string | Yes | Transaction hash |

#### Request Example
```bash
curl -X GET http://localhost:8000/api/v1/blockchain/transaction/0x5a7d7c7e8f3b2a1c9d4e6f8a9b2c3d5e7f9a1b3c5d7e9f2a4b6c8d1e3f5a7b9c1d \
  -H "Authorization: Bearer <your-jwt-token>"
```

#### Response Example
```json
{
  "block_number": 1000000,
  "from": "0x...",
  "gas_limit": 21000,
  "gas_price": "20000000000",
  "hash": "0x5a7d7c7e8f3b2a1c9d4e6f8a9b2c3d5e7f9a1b3c5d7e9f2a4b6c8d1e3f5a7b9c1d",
  "nonce": 0,
  "status": "confirmed",
  "timestamp": 1758550123,
  "to": "0x...",
  "value": "1000000000000000000"
}
```

#### Response Fields
| Field | Type | Description |
|-------|------|-------------|
| `hash` | string | Transaction hash |
| `from` | string | Sender address |
| `to` | string | Recipient address |
| `value` | string | Transfer amount (Wei) |
| `gas_limit` | number | Gas limit |
| `gas_price` | string | Gas price |
| `nonce` | number | Transaction nonce |
| `block_number` | number | Block number |
| `status` | string | Transaction status |
| `timestamp` | number | Transaction timestamp |

---

### 6. Get Block Information

Get block information by block number.

**Endpoint**: `GET /api/v1/blockchain/block/{number}`

#### Path Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `number` | string/number | Yes | Block number or "latest" |

#### Request Examples
```bash
# Get latest block
curl -X GET http://localhost:8000/api/v1/blockchain/block/latest \
  -H "Authorization: Bearer <your-jwt-token>"

# Get specific block
curl -X GET http://localhost:8000/api/v1/blockchain/block/999999 \
  -H "Authorization: Bearer <your-jwt-token>"
```

#### Response Example
```json
{
  "current": 1000000,
  "number": 1000000,
  "status": "available",
  "timestamp": 1758550166
}
```

#### Response Fields
| Field | Type | Description |
|-------|------|-------------|
| `number` | number | Requested block number |
| `current` | number | Current latest block number |
| `status` | string | Block status |
| `timestamp` | number | Query timestamp |

---

## Error Responses

API uses standard HTTP status codes to indicate request results:

| Status Code | Description |
|-------------|-------------|
| 200 | Success |
| 400 | Bad request parameters |
| 401 | Unauthorized |
| 404 | Resource not found |
| 500 | Internal server error |
| 503 | Service unavailable |

### Error Response Format
```json
{
  "error": "Error description message"
}
```

## Usage Examples

### JavaScript/Node.js
```javascript
const axios = require('axios');

const api = axios.create({
  baseURL: 'http://localhost:8000',
  headers: {
    'Authorization': 'Bearer your-jwt-token'
  }
});

// Get chain status
const status = await api.get('/api/v1/blockchain/status');
console.log('Chain status:', status.data);

// Query balance
const balance = await api.get('/api/v1/blockchain/balance/0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5');
console.log('Balance:', balance.data.eth, 'ISA');

// Send transaction
const tx = await api.post('/api/v1/blockchain/transaction', {
  to: '0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5',
  value: '1000000000000000000'
});
console.log('Transaction hash:', tx.data.transaction_hash);
```

### Python
```python
import requests

headers = {'Authorization': 'Bearer your-jwt-token'}
base_url = 'http://localhost:8000'

# Get chain status
response = requests.get(f'{base_url}/api/v1/blockchain/status', headers=headers)
status = response.json()
print(f"Current block: {status['block_number']}")

# Query balance
address = '0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5'
response = requests.get(f'{base_url}/api/v1/blockchain/balance/{address}', headers=headers)
balance = response.json()
print(f"Balance: {balance['eth']} ISA")

# Send transaction
tx_data = {
    'to': '0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb5',
    'value': '1000000000000000000'
}
response = requests.post(f'{base_url}/api/v1/blockchain/transaction', 
                        json=tx_data, headers=headers)
tx = response.json()
print(f"Transaction hash: {tx['transaction_hash']}")
```

## Important Notes

1. **Amount Units**: All amounts are in Wei as base unit (1 ISA = 10^18 Wei)
2. **Address Format**: Wallet addresses must be valid Ethereum format (starting with 0x)
3. **Gas Fees**: Set appropriate gas limit and price when sending transactions
4. **Error Handling**: Properly handle API error responses
5. **Authentication**: Most endpoints require valid JWT tokens

## Changelog

### v1.0.0 (2025-09-22)
- Initial release
- Support for basic blockchain operations
- Unified authentication mechanism
- Complete error handling