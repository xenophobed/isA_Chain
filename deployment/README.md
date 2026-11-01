# isA_Chain Deployment

## Local Development
```bash
docker-compose up -d
```

## Kubernetes Deployment (Aligned with isA_Cloud)

isA_Chain 采用标准 `isA_Cloud/deployments/charts/isa-service` 通用 Helm 规范进行服务与存储管理。

### 1. 验证 Helm 渲染
```bash
helm template blockchain-node ../isA_Cloud/deployments/charts/isa-service \
  -f deployment/helm/values.yaml \
  -f deployment/helm/values-staging.yaml
```

### 2. 部署到 Staging
```bash
helm upgrade --install blockchain-node \
  ../isA_Cloud/deployments/charts/isa-service \
  -f deployment/helm/values.yaml \
  -f deployment/helm/values-staging.yaml \
  -n isa-cloud-staging
```

### 3. 配置说明
| Env Var | Default (Staging) | Description |
|---------|-------------------|-------------|
| CHAIN_ID | 15490 | Testnet Chain ID (15489 for mainnet) |
| RPC_PORT | 9944 | JSON-RPC / WebSocket 端口 |
| BLOCK_TIME_SECS | 3 | 出块间隔时间（秒） |
| DATA_DIR | /data | RocksDB 数据持久化挂载目录 |
| PERSIST | true | 启用 RocksDB 状态落地 |
| NATS_URL | nats://nats...:4222 | 云端 NATS 事件结算总线 |
