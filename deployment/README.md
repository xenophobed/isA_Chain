# isA_Chain Deployment

## Local Development
docker-compose up -d

## Kubernetes (via isA_Cloud)

### Build & Push
./deployment/scripts/build.sh v0.1.0
docker push harbor.local:30443/isa/blockchain-node:v0.1.0

### Deploy to Staging
helm upgrade --install blockchain-node \
  ~/Documents/Fun/isA/isA_Cloud/deployments/charts/isa-service \
  -f deployment/helm/values.yaml \
  -f deployment/helm/values-staging.yaml \
  -n isa-cloud-staging

### Deploy via ArgoCD
argocd app sync blockchain-node-staging

## Configuration
| Env Var | Default | Description |
|---------|---------|-------------|
| CHAIN_ID | 15489 | Chain ID (15490 for testnet) |
| RPC_PORT | 9944 | RPC server port |
| BLOCK_TIME_SECS | 3 | Block production interval |
| DATA_DIR | /data | RocksDB data directory |
| PERSIST | true | Enable RocksDB persistence |
| NATS_URL | nats://localhost:4222 | NATS server for settlement |
