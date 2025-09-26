# isA_Chain 混合服务架构设计规范

## 概述

本文档定义了 isA_Chain 生态系统中智能合约 HTTP 适配器的标准架构设计，通过混合架构模式将区块链功能以标准 RESTful API 形式暴露给开发者和应用。

## 架构原则

### 核心原则：按功能域分离 + 分层架构

```
┌─────────────────────────────────────────────────────────┐
│                    Gateway (8000)                       │
│  ✅ 路由转发    ✅ 认证鉴权    ✅ 限流    ✅ 监控       │
└─────────────────────────────────────────────────────────┘
                             │
              ┌─────────────────────────────────┐
              │         服务发现 (Consul)         │
              └─────────────────────────────────┘
                             │
    ┌────────────┬────────────┼────────────┬────────────┐
    │            │            │            │            │
┌───▼───┐   ┌───▼───┐   ┌───▼───┐   ┌───▼───┐   ┌───▼───┐
│ DeFi  │   │  NFT  │   │ Oracle│   │Privacy│   │ Tools │
│Service│   │Service│   │Service│   │Service│   │Service│
│(8311) │   │(8312) │   │(8313) │   │(8314) │   │(8315) │
└───────┘   └───────┘   └───────┘   └───────┘   └───────┘
    │            │            │            │            │
    ▼            ▼            ▼            ▼            ▼
┌───────────────────────────────────────────────────────┐
│              Blockchain Core (RPC)                    │
│  SimpleDEX │ ISANFT │ PriceOracle │ PrivacyPool │...  │
└───────────────────────────────────────────────────────┘
```

## 三层架构设计

### Layer 1: 业务微服务层

每个功能域独立部署为微服务，负责特定领域的业务逻辑处理。

#### 服务端口分配

| 服务名称 | 端口 | 功能域 | 对应合约 |
|---------|------|--------|---------|
| defi-service | 8311 | DeFi操作 | SimpleDEX, StakingPool, YieldFarming, LendingProtocol |
| nft-service | 8312 | NFT管理 | ISANFT, NFTMarketplace |
| oracle-service | 8313 | 预言机服务 | PriceOracle |
| privacy-service | 8314 | 隐私交易 | PrivacyPool |
| tools-service | 8315 | 开发工具 | 通用合约调用 |
| exchange-service | 8316 | 交易所 | SpotExchange, OrderManager |

#### 服务实现标准

```typescript
// 标准服务结构
service-name/
├── package.json              # 项目配置
├── .env.example             # 环境变量示例
├── README.md                # 服务文档
├── tsconfig.json            # TypeScript配置
└── src/
    ├── index.ts             # 服务入口
    ├── config.ts            # 配置管理
    ├── controllers/         # HTTP控制器
    │   └── domain.controller.ts
    ├── services/            # 业务逻辑
    │   ├── blockchain.service.ts
    │   └── contracts/       # 合约服务
    │       ├── contract1.service.ts
    │       └── contract2.service.ts
    ├── utils/               # 工具函数
    │   ├── logger.ts
    │   └── validators.ts
    ├── middleware/          # 中间件
    │   └── auth.middleware.ts
    └── abi/                 # 合约ABI文件
        └── Contract.json
```

### Layer 2: 工具服务层

为开发者提供通用的合约交互能力。

```javascript
// tools-service 通用合约调用接口
POST /api/v1/contract/call
{
    "contract": "0x...",     // 合约地址
    "method": "transfer",    // 方法名
    "params": [...],         // 参数数组
    "gasLimit": 500000,      // Gas限制
    "value": "0"            // ETH值（Wei）
}

POST /api/v1/contract/read
{
    "contract": "0x...",
    "method": "balanceOf",
    "params": ["0xUserAddress"]
}

POST /api/v1/abi/encode
{
    "abi": [...],
    "method": "transfer",
    "params": ["0x...", "1000000"]
}
```

### Layer 3: Gateway适配层

Gateway保持轻量，只负责核心功能的直接路由。

```go
// Gateway核心路由配置
func (g *Gateway) SetupHTTPRoutes() *gin.Engine {
    // 基础功能直接路由
    blockchainAPI.GET("/balance/:address", g.getBalance)
    blockchainAPI.POST("/transaction", g.sendTransaction)
    blockchainAPI.GET("/status", g.blockchainStatus)
    
    // 复杂业务代理到微服务
    router.NoRoute(g.dynamicProxy.Handler())
}
```

## API设计规范

### RESTful接口标准

所有服务必须遵循以下RESTful设计规范：

```yaml
# API路径规范
GET    /api/v1/resources       # 获取资源列表
GET    /api/v1/resources/:id   # 获取单个资源
POST   /api/v1/resources       # 创建资源
PUT    /api/v1/resources/:id   # 更新资源
DELETE /api/v1/resources/:id   # 删除资源

# 响应格式标准
{
  "success": true,
  "data": {
    // 实际数据
  },
  "timestamp": 1234567890,
  "message": "操作成功"
}

# 错误响应格式
{
  "success": false,
  "error": {
    "code": "INSUFFICIENT_FUNDS",
    "message": "余额不足",
    "details": {}
  },
  "timestamp": 1234567890
}
```

### 状态码使用规范

| 状态码 | 用途 |
|--------|------|
| 200 | 成功 |
| 201 | 创建成功 |
| 400 | 请求参数错误 |
| 401 | 未授权 |
| 403 | 禁止访问 |
| 404 | 资源不存在 |
| 429 | 请求过于频繁 |
| 500 | 服务器内部错误 |
| 503 | 服务不可用 |

## 服务实现最佳实践

### 1. 服务注册与发现

所有服务启动时必须注册到 Consul：

```typescript
// Consul注册示例
import Consul from 'consul';

const consul = new Consul({
  host: 'localhost',
  port: 8500
});

await consul.agent.service.register({
  name: 'defi-service',
  id: 'defi-service-1',
  address: 'localhost',
  port: 8311,
  tags: ['blockchain', 'defi', 'http'],
  check: {
    http: 'http://localhost:8311/health',
    interval: '10s',
    timeout: '5s'
  }
});
```

### 2. 健康检查端点

每个服务必须实现健康检查端点：

```typescript
app.get('/health', (req, res) => {
  res.json({
    status: 'healthy',
    service: 'defi-service',
    version: '1.0.0',
    uptime: process.uptime(),
    timestamp: new Date().toISOString()
  });
});
```

### 3. 错误处理

统一的错误处理机制：

```typescript
class ServiceError extends Error {
  constructor(
    public code: string,
    public message: string,
    public statusCode: number = 500,
    public details?: any
  ) {
    super(message);
  }
}

// 使用示例
throw new ServiceError(
  'INSUFFICIENT_BALANCE',
  '余额不足',
  400,
  { required: '1000', available: '500' }
);
```

### 4. 日志规范

使用结构化日志：

```typescript
import winston from 'winston';

const logger = winston.createLogger({
  format: winston.format.json(),
  transports: [
    new winston.transports.Console(),
    new winston.transports.File({ filename: 'service.log' })
  ]
});

// 使用示例
logger.info('Transaction sent', {
  service: 'defi-service',
  action: 'swap',
  txHash: '0x...',
  userId: 'user123'
});
```

### 5. 配置管理

使用环境变量和配置文件：

```typescript
// config.ts
export const config = {
  server: {
    port: parseInt(process.env.PORT || '8311'),
    environment: process.env.NODE_ENV || 'development'
  },
  blockchain: {
    rpcUrl: process.env.BLOCKCHAIN_RPC_URL || 'http://localhost:8545',
    chainId: parseInt(process.env.CHAIN_ID || '1337')
  },
  contracts: {
    simpleDEX: process.env.CONTRACT_SIMPLE_DEX || '0x...'
  }
};
```

### 6. 限流策略

实现请求限流保护：

```typescript
import rateLimit from 'express-rate-limit';

const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15分钟
  max: 100, // 最多100个请求
  message: '请求过于频繁，请稍后再试'
});

app.use('/api/', limiter);
```

### 7. 事务处理

支持多步骤操作的事务管理：

```typescript
async function executeComplexOperation(params) {
  const operations = [];
  
  try {
    // 步骤1：检查余额
    const balance = await checkBalance(params.address);
    operations.push({ step: 'checkBalance', success: true });
    
    // 步骤2：执行交易
    const txHash = await sendTransaction(params);
    operations.push({ step: 'sendTransaction', txHash });
    
    // 步骤3：等待确认
    const receipt = await waitForConfirmation(txHash);
    operations.push({ step: 'confirmation', receipt });
    
    return { success: true, operations };
  } catch (error) {
    // 回滚逻辑
    await rollback(operations);
    throw error;
  }
}
```

## 部署架构

### 容器化部署

每个服务应该容器化：

```dockerfile
# Dockerfile示例
FROM node:18-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY . .
EXPOSE 8311
CMD ["npm", "start"]
```

### Docker Compose编排

```yaml
version: '3.8'

services:
  gateway:
    image: isa-cloud/gateway:latest
    ports:
      - "8000:8000"
    depends_on:
      - consul
      
  defi-service:
    image: isa-chain/defi-service:latest
    ports:
      - "8311:8311"
    environment:
      - BLOCKCHAIN_RPC_URL=http://blockchain:8545
      - CONSUL_HOST=consul
    depends_on:
      - consul
      - blockchain
      
  nft-service:
    image: isa-chain/nft-service:latest
    ports:
      - "8312:8312"
    environment:
      - BLOCKCHAIN_RPC_URL=http://blockchain:8545
      - CONSUL_HOST=consul
    depends_on:
      - consul
      - blockchain
      
  consul:
    image: consul:latest
    ports:
      - "8500:8500"
      - "8600:8600/udp"
      
  blockchain:
    image: isa-chain/node:latest
    ports:
      - "8545:8545"
```

## 监控与可观测性

### Metrics收集

```typescript
import promClient from 'prom-client';

// 创建指标
const httpRequestDuration = new promClient.Histogram({
  name: 'http_request_duration_seconds',
  help: 'Duration of HTTP requests in seconds',
  labelNames: ['method', 'route', 'status']
});

// 中间件收集指标
app.use((req, res, next) => {
  const end = httpRequestDuration.startTimer();
  res.on('finish', () => {
    end({ method: req.method, route: req.path, status: res.statusCode });
  });
  next();
});

// 暴露指标端点
app.get('/metrics', async (req, res) => {
  res.set('Content-Type', promClient.register.contentType);
  res.end(await promClient.register.metrics());
});
```

### 分布式追踪

集成OpenTelemetry：

```typescript
import { NodeTracerProvider } from '@opentelemetry/sdk-trace-node';
import { registerInstrumentations } from '@opentelemetry/instrumentation';

const provider = new NodeTracerProvider();
provider.register();

registerInstrumentations({
  instrumentations: [
    new HttpInstrumentation(),
    new ExpressInstrumentation()
  ]
});
```

## 安全规范

### 1. 私钥管理

- 永远不要硬编码私钥
- 使用环境变量或密钥管理服务
- 支持硬件钱包集成

### 2. 输入验证

```typescript
import Joi from 'joi';

const swapSchema = Joi.object({
  tokenIn: Joi.string().pattern(/^0x[a-fA-F0-9]{40}$/).required(),
  tokenOut: Joi.string().pattern(/^0x[a-fA-F0-9]{40}$/).required(),
  amountIn: Joi.string().pattern(/^\d+(\.\d+)?$/).required(),
  minAmountOut: Joi.string().pattern(/^\d+(\.\d+)?$/).required(),
  userAddress: Joi.string().pattern(/^0x[a-fA-F0-9]{40}$/).required()
});

// 验证输入
const { error, value } = swapSchema.validate(req.body);
if (error) {
  return res.status(400).json({ error: error.details });
}
```

### 3. 签名验证

验证请求签名确保请求来源可信：

```typescript
function verifySignature(message: string, signature: string, address: string): boolean {
  const recoveredAddress = ethers.verifyMessage(message, signature);
  return recoveredAddress.toLowerCase() === address.toLowerCase();
}
```

## 测试规范

### 单元测试

```typescript
describe('DeFi Service', () => {
  describe('Swap', () => {
    it('should calculate correct swap amount', async () => {
      const quote = await service.getSwapQuote('tokenA', 'tokenB', '100');
      expect(quote.estimatedOut).toBe('95');
      expect(quote.priceImpact).toBeLessThan(5);
    });
  });
});
```

### 集成测试

```typescript
describe('E2E Tests', () => {
  it('should complete full swap flow', async () => {
    // 1. 获取报价
    const quote = await request(app)
      .post('/api/v1/swap/quote')
      .send({ tokenIn, tokenOut, amountIn });
      
    // 2. 执行交换
    const swap = await request(app)
      .post('/api/v1/swap/execute')
      .send({ ...quote.data, userAddress });
      
    // 3. 验证交易
    expect(swap.body.transactionHash).toMatch(/^0x[a-fA-F0-9]{64}$/);
  });
});
```

## 版本管理

### API版本控制

```
/api/v1/...  # 当前稳定版本
/api/v2/...  # 新版本（向后兼容）
/api/beta/...  # 实验性功能
```

### 变更日志

维护详细的CHANGELOG.md：

```markdown
# Changelog

## [1.2.0] - 2024-01-15
### Added
- 新增批量交换功能
- 支持EIP-1559交易

### Changed
- 优化Gas估算算法

### Fixed
- 修复滑点计算错误
```

## 实施路线图

### Phase 1: 核心服务（第1-2周）
- [ ] DeFi Service - DeFi核心功能
- [ ] NFT Service - NFT铸造和交易
- [ ] Tools Service - 开发工具

### Phase 2: 扩展服务（第3-4周）
- [ ] Oracle Service - 预言机服务
- [ ] Privacy Service - 隐私交易
- [ ] Exchange Service - 交易所功能

### Phase 3: 优化与监控（第5-6周）
- [ ] 性能优化
- [ ] 监控系统集成
- [ ] 自动化测试完善

### Phase 4: 生产部署（第7-8周）
- [ ] 安全审计
- [ ] 负载测试
- [ ] 主网部署

## 总结

这套混合架构设计实现了：

1. **解耦性**：各服务独立开发、部署、扩展
2. **可维护性**：清晰的分层和职责划分
3. **可扩展性**：易于添加新功能和服务
4. **标准化**：统一的API设计和实现规范
5. **可观测性**：完善的监控和日志体系
6. **安全性**：多层安全防护机制

通过遵循这套架构规范，可以将区块链的复杂性封装在服务层，为上层应用提供简洁、标准的HTTP API接口，实现Web3功能的Web2化体验。