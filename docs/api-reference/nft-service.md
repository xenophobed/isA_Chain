# NFT Service API Reference

## Overview
The NFT Service provides HTTP APIs for NFT operations including minting, collection management, marketplace functions, royalties, and metadata management.

**Base URL**: `http://localhost:8312`  
**Gateway URL**: `http://localhost:8000/api/v1/nft-service`

## Health Check

### GET /health
Check service health status.

**Request:**
```bash
curl -X GET http://localhost:8312/health
```

**Response:**
```json
{
  "status": "healthy",
  "service": "nft-service",
  "version": "1.0.0",
  "timestamp": "2025-09-25T02:31:05.766Z"
}
```

## Collection Management

### GET /api/v1/collections
Get all NFT collections.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/collections
```

**Response:**
```json
{
  "success": true,
  "data": {
    "collections": [
      {
        "address": "0x...",
        "name": "Cool NFT Collection",
        "symbol": "COOL",
        "totalSupply": 1000,
        "owner": "0x...",
        "metadata": {
          "description": "A collection of cool NFTs",
          "image": "ipfs://...",
          "externalUrl": "https://example.com"
        }
      }
    ]
  },
  "pagination": {
    "page": 1,
    "limit": 20
  }
}
```

### POST /api/v1/collections
Create a new NFT collection.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/collections \
  -H "Content-Type: application/json" \
  -d '{
    "name": "My NFT Collection",
    "symbol": "MNC",
    "maxSupply": 10000,
    "baseURI": "ipfs://QmXxx/",
    "royaltyBps": 250
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "collectionAddress": "0xabc...",
  "data": {
    "name": "My NFT Collection",
    "symbol": "MNC",
    "maxSupply": 10000
  }
}
```

### GET /api/v1/collections/:address
Get details of a specific collection.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/collections/0xabc...
```

**Response:**
```json
{
  "success": true,
  "collection": {
    "address": "0xabc...",
    "name": "My NFT Collection",
    "symbol": "MNC",
    "totalSupply": 150,
    "maxSupply": 10000,
    "owner": "0x...",
    "baseURI": "ipfs://QmXxx/",
    "royaltyBps": 250,
    "stats": {
      "floorPrice": "1000000000000000000",
      "volume24h": "5000000000000000000",
      "holders": 75
    }
  }
}
```

## NFT Minting

### POST /api/v1/mint
Mint a new NFT.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/mint \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "0xabc...",
    "to": "0x123...",
    "name": "Cool NFT #1",
    "description": "This is a cool NFT",
    "image": "ipfs://QmYxxx/image.png",
    "attributes": [
      {
        "trait_type": "Background",
        "value": "Blue"
      },
      {
        "trait_type": "Rarity",
        "value": "Rare"
      }
    ]
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x456...",
  "tokenId": "1",
  "metadata": {
    "name": "Cool NFT #1",
    "description": "This is a cool NFT",
    "image": "ipfs://QmYxxx/image.png",
    "attributes": [...]
  }
}
```

### POST /api/v1/batch-mint
Batch mint multiple NFTs.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/batch-mint \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "0xabc...",
    "recipients": ["0x111...", "0x222...", "0x333..."],
    "metadataURIs": ["ipfs://Qm1/", "ipfs://Qm2/", "ipfs://Qm3/"]
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x789...",
  "tokenIds": ["1", "2", "3"],
  "count": 3
}
```

## NFT Transfers

### POST /api/v1/transfer
Transfer an NFT.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/transfer \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "0xabc...",
    "tokenId": "1",
    "from": "0x111...",
    "to": "0x222..."
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0xdef...",
  "tokenId": "1",
  "from": "0x111...",
  "to": "0x222..."
}
```

## NFT Queries

### GET /api/v1/nfts/:address
Get NFTs owned by an address.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/nfts/0x123...?page=1&limit=20
```

**Response:**
```json
{
  "success": true,
  "nfts": [
    {
      "collection": "0xabc...",
      "tokenId": "1",
      "name": "Cool NFT #1",
      "description": "This is a cool NFT",
      "image": "ipfs://QmYxxx/image.png",
      "attributes": [...],
      "owner": "0x123..."
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 5
  }
}
```

### GET /api/v1/nft/:collection/:tokenId
Get details of a specific NFT.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/nft/0xabc.../1
```

**Response:**
```json
{
  "success": true,
  "nft": {
    "collection": "0xabc...",
    "tokenId": "1",
    "owner": "0x123...",
    "metadata": {
      "name": "Cool NFT #1",
      "description": "This is a cool NFT",
      "image": "ipfs://QmYxxx/image.png",
      "attributes": [...]
    },
    "history": [
      {
        "event": "Transfer",
        "from": "0x000...",
        "to": "0x123...",
        "timestamp": 1234567890,
        "txHash": "0x..."
      }
    ]
  }
}
```

## Marketplace Endpoints

### GET /api/v1/marketplace/listings
Get active marketplace listings.

**Request:**
```bash
curl -X GET "http://localhost:8312/api/v1/marketplace/listings?collection=0xabc...&minPrice=0&maxPrice=1000000000000000000"
```

**Response:**
```json
{
  "success": true,
  "listings": [
    {
      "id": "1",
      "collection": "0xabc...",
      "tokenId": "1",
      "seller": "0x111...",
      "price": "1000000000000000000",
      "currency": "ETH",
      "expiry": 1234567890,
      "status": "active"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20
  }
}
```

### POST /api/v1/marketplace/list
List an NFT for sale.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/marketplace/list \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "0xabc...",
    "tokenId": "1",
    "price": "1000000000000000000",
    "currency": "ETH",
    "duration": 86400
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x123...",
  "listingId": "1",
  "data": {
    "collection": "0xabc...",
    "tokenId": "1",
    "price": "1000000000000000000",
    "expiry": 1234567890
  }
}
```

### POST /api/v1/marketplace/buy
Buy an NFT from marketplace.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/marketplace/buy \
  -H "Content-Type: application/json" \
  -d '{
    "listingId": "1"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x456...",
  "tokenId": "1",
  "price": "1000000000000000000",
  "buyer": "0x222...",
  "seller": "0x111..."
}
```

### POST /api/v1/marketplace/cancel
Cancel a marketplace listing.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/marketplace/cancel \
  -H "Content-Type: application/json" \
  -d '{
    "listingId": "1"
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0x789...",
  "listingId": "1"
}
```

### POST /api/v1/marketplace/offer
Make an offer on an NFT.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/marketplace/offer \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "0xabc...",
    "tokenId": "1",
    "price": "900000000000000000",
    "duration": 86400
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0xabc...",
  "offerId": "1",
  "data": {
    "collection": "0xabc...",
    "tokenId": "1",
    "price": "900000000000000000",
    "offeror": "0x333...",
    "expiry": 1234567890
  }
}
```

## Metadata Management

### POST /api/v1/metadata/upload
Upload metadata to IPFS.

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/metadata/upload \
  -H "Content-Type: application/json" \
  -d '{
    "name": "Cool NFT",
    "description": "A very cool NFT",
    "image": "ipfs://QmImage...",
    "attributes": [
      {
        "trait_type": "Background",
        "value": "Blue"
      }
    ]
  }'
```

**Response:**
```json
{
  "success": true,
  "ipfsHash": "QmMetadata123...",
  "url": "ipfs://QmMetadata123..."
}
```

### GET /api/v1/metadata/:ipfsHash
Retrieve metadata from IPFS.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/metadata/QmMetadata123...
```

**Response:**
```json
{
  "success": true,
  "metadata": {
    "name": "Cool NFT",
    "description": "A very cool NFT",
    "image": "ipfs://QmImage...",
    "attributes": [...]
  }
}
```

## Royalties

### GET /api/v1/royalties/:collection
Get royalty information for a collection.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/royalties/0xabc...
```

**Response:**
```json
{
  "success": true,
  "royalties": {
    "receiver": "0x111...",
    "percentage": 2.5,
    "basisPoints": 250
  }
}
```

### POST /api/v1/royalties/update
Update royalty settings (owner only).

**Request:**
```bash
curl -X POST http://localhost:8312/api/v1/royalties/update \
  -H "Content-Type: application/json" \
  -d '{
    "collection": "0xabc...",
    "receiver": "0x222...",
    "basisPoints": 500
  }'
```

**Response:**
```json
{
  "success": true,
  "txHash": "0xdef...",
  "royalties": {
    "receiver": "0x222...",
    "basisPoints": 500
  }
}
```

## Analytics

### GET /api/v1/analytics/collection/:address
Get collection analytics.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/analytics/collection/0xabc...
```

**Response:**
```json
{
  "success": true,
  "analytics": {
    "floorPrice": "1000000000000000000",
    "ceilingPrice": "5000000000000000000",
    "volume24h": "25000000000000000000",
    "volume7d": "150000000000000000000",
    "sales24h": 25,
    "sales7d": 150,
    "holders": 75,
    "uniqueHolders": 72,
    "listed": 15,
    "marketCap": "100000000000000000000"
  }
}
```

### GET /api/v1/analytics/trending
Get trending collections.

**Request:**
```bash
curl -X GET http://localhost:8312/api/v1/analytics/trending?period=24h
```

**Response:**
```json
{
  "success": true,
  "trending": [
    {
      "collection": "0xabc...",
      "name": "Cool Collection",
      "volume24h": "50000000000000000000",
      "volumeChange": 125.5,
      "floorPrice": "1000000000000000000",
      "sales": 50
    }
  ]
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
- `404 Not Found` - NFT or collection not found
- `403 Forbidden` - Not authorized for this operation
- `500 Internal Server Error` - Server error

## Rate Limiting

The API implements rate limiting:
- **Limit**: 200 requests per 15 minutes per IP
- **Headers**: Rate limit information is included in response headers

## Notes

- All prices are in wei (1 ETH = 10^18 wei)
- Timestamps are Unix timestamps in seconds
- IPFS hashes should be valid CID v0 or v1
- Collection addresses must be valid Ethereum addresses
- The service requires connection to IPFS for metadata operations
- Marketplace operations require approval for the marketplace contract