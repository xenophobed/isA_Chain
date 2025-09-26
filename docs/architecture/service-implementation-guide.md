# isA_Chain 服务实施指南

## 快速开始

本指南帮助开发者快速创建符合 isA_Chain 架构规范的微服务。

## 创建新服务的步骤

### 1. 使用服务模板

```bash
# 复制DeFi服务作为模板
cp -r dapp/backend/defi-service dapp/backend/your-service

# 进入服务目录
cd dapp/backend/your-service

# 更新package.json中的服务名称
sed -i 's/defi-service/your-service/g' package.json
```

### 2. 服务配置清单

每个服务需要配置以下内容：

#### 环境变量配置 (.env)

```env
# 服务配置
SERVICE_NAME=your-service
SERVICE_PORT=83XX  # 使用分配的端口
NODE_ENV=development

# 区块链配置
BLOCKCHAIN_RPC_URL=http://localhost:8545
CHAIN_ID=1337
PRIVATE_KEY=0x...  # 服务账户私钥

# 合约地址
CONTRACT_ADDRESS_1=0x...
CONTRACT_ADDRESS_2=0x...

# Consul配置
CONSUL_HOST=localhost
CONSUL_PORT=8500

# Gateway配置
GATEWAY_URL=http://localhost:8000

# 日志配置
LOG_LEVEL=info
LOG_FORMAT=json
```

#### TypeScript配置 (tsconfig.json)

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "lib": ["ES2020"],
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "resolveJsonModule": true,
    "declaration": true,
    "declarationMap": true,
    "sourceMap": true
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist", "**/*.test.ts"]
}
```

### 3. 核心服务文件

#### 主入口文件 (src/index.ts)

```typescript
import express from 'express';
import cors from 'cors';
import helmet from 'helmet';
import { ConsulService } from './services/consul.service';
import { BlockchainService } from './services/blockchain.service';
import { YourController } from './controllers/your.controller';
import { logger } from './utils/logger';
import { config } from './config';

const app = express();
const PORT = config.server.port;

// 中间件
app.use(helmet());
app.use(cors());
app.use(express.json());

// 健康检查
app.get('/health', (req, res) => {
  res.json({
    status: 'healthy',
    service: config.server.name,
    version: '1.0.0',
    timestamp: new Date().toISOString()
  });
});

async function bootstrap() {
  try {
    // 初始化区块链连接
    const blockchain = await BlockchainService.getInstance();
    
    // 初始化控制器
    const controller = new YourController(blockchain);
    
    // 注册路由
    const router = express.Router();
    // 添加你的路由...
    app.use('/api/v1', router);
    
    // 注册到Consul
    const consul = new ConsulService(config.consul);
    await consul.register({
      name: config.server.name,
      port: PORT,
      tags: ['blockchain', 'your-domain'],
      check: {
        http: `http://localhost:${PORT}/health`,
        interval: '10s'
      }
    });
    
    // 启动服务
    app.listen(PORT, () => {
      logger.info(`Service started on port ${PORT}`);
    });
  } catch (error) {
    logger.error('Failed to start service', error);
    process.exit(1);
  }
}

bootstrap();
```

#### Consul服务 (src/services/consul.service.ts)

```typescript
import Consul from 'consul';
import { logger } from '../utils/logger';

export class ConsulService {
  private consul: Consul.Consul;
  private serviceId: string;

  constructor(config: any) {
    this.consul = new Consul({
      host: config.host,
      port: config.port,
      secure: config.secure
    });
    this.serviceId = '';
  }

  async register(options: any) {
    try {
      this.serviceId = `${options.name}-${Date.now()}`;
      
      await this.consul.agent.service.register({
        id: this.serviceId,
        name: options.name,
        address: 'localhost',
        port: options.port,
        tags: options.tags,
        check: options.check
      });
      
      logger.info(`Service registered with Consul: ${this.serviceId}`);
    } catch (error) {
      logger.error('Failed to register with Consul', error);
      throw error;
    }
  }

  async deregister() {
    if (this.serviceId) {
      await this.consul.agent.service.deregister(this.serviceId);
      logger.info(`Service deregistered from Consul: ${this.serviceId}`);
    }
  }
}
```

#### 日志工具 (src/utils/logger.ts)

```typescript
import winston from 'winston';

export const logger = winston.createLogger({
  level: process.env.LOG_LEVEL || 'info',
  format: winston.format.combine(
    winston.format.timestamp(),
    winston.format.errors({ stack: true }),
    winston.format.json()
  ),
  transports: [
    new winston.transports.Console({
      format: winston.format.combine(
        winston.format.colorize(),
        winston.format.simple()
      )
    }),
    new winston.transports.File({
      filename: 'error.log',
      level: 'error'
    }),
    new winston.transports.File({
      filename: 'combined.log'
    })
  ]
});
```

## 服务开发检查清单

### 必须实现

- [ ] 健康检查端点 `/health`
- [ ] Consul服务注册
- [ ] 结构化日志
- [ ] 错误处理
- [ ] 环境变量配置
- [ ] TypeScript类型定义
- [ ] 单元测试
- [ ] README文档

### 建议实现

- [ ] 请求限流
- [ ] 指标收集 `/metrics`
- [ ] 请求追踪
- [ ] 缓存机制
- [ ] 重试机制
- [ ] 断路器模式
- [ ] API文档 (Swagger/OpenAPI)
- [ ] 集成测试

## 合约集成示例

### 1. 添加合约ABI

将合约ABI文件放在 `src/abi/` 目录：

```json
// src/abi/YourContract.json
{
  "abi": [
    {
      "inputs": [],
      "name": "yourMethod",
      "outputs": [{"type": "uint256"}],
      "stateMutability": "view",
      "type": "function"
    }
  ]
}
```

### 2. 创建合约服务

```typescript
// src/services/contracts/yourContract.service.ts
import { ethers } from 'ethers';
import { BlockchainService } from '../blockchain.service';
import YourContractABI from '../../abi/YourContract.json';
import { config } from '../../config';

export class YourContractService {
  private contract: ethers.Contract | null = null;

  constructor(private blockchain: BlockchainService) {}

  private async getContract(): Promise<ethers.Contract> {
    if (!this.contract) {
      this.contract = await this.blockchain.getContract(
        config.contracts.yourContract,
        YourContractABI.abi
      );
    }
    return this.contract;
  }

  async yourMethod(): Promise<any> {
    const contract = await this.getContract();
    return await contract.yourMethod();
  }

  async sendTransaction(params: any): Promise<string> {
    const contract = await this.getContract();
    const tx = await contract.yourTransactionMethod(params);
    return tx.hash;
  }
}
```

### 3. 创建控制器

```typescript
// src/controllers/your.controller.ts
import { Request, Response } from 'express';
import { YourContractService } from '../services/contracts/yourContract.service';
import { logger } from '../utils/logger';

export class YourController {
  private contractService: YourContractService;

  constructor(blockchain: BlockchainService) {
    this.contractService = new YourContractService(blockchain);
  }

  async handleRequest(req: Request, res: Response) {
    try {
      const result = await this.contractService.yourMethod();
      res.json({
        success: true,
        data: result,
        timestamp: Date.now()
      });
    } catch (error) {
      logger.error('Request failed', error);
      res.status(500).json({
        success: false,
        error: 'Internal server error'
      });
    }
  }
}
```

## 测试策略

### 单元测试示例

```typescript
// src/__tests__/services/yourContract.service.test.ts
import { YourContractService } from '../../services/contracts/yourContract.service';

describe('YourContractService', () => {
  let service: YourContractService;
  let mockBlockchain: any;

  beforeEach(() => {
    mockBlockchain = {
      getContract: jest.fn().mockResolvedValue({
        yourMethod: jest.fn().mockResolvedValue('test result')
      })
    };
    service = new YourContractService(mockBlockchain);
  });

  it('should call contract method', async () => {
    const result = await service.yourMethod();
    expect(result).toBe('test result');
  });
});
```

### 集成测试示例

```typescript
// src/__tests__/integration/api.test.ts
import request from 'supertest';
import { app } from '../../index';

describe('API Integration Tests', () => {
  it('should return health status', async () => {
    const response = await request(app)
      .get('/health')
      .expect(200);
      
    expect(response.body.status).toBe('healthy');
  });

  it('should handle API request', async () => {
    const response = await request(app)
      .post('/api/v1/your-endpoint')
      .send({ param: 'value' })
      .expect(200);
      
    expect(response.body.success).toBe(true);
  });
});
```

## 部署准备

### 1. Dockerfile

```dockerfile
FROM node:18-alpine AS builder
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM node:18-alpine
WORKDIR /app
COPY package*.json ./
RUN npm ci --only=production
COPY --from=builder /app/dist ./dist
EXPOSE 8300
CMD ["npm", "start"]
```

### 2. 健康检查脚本

```bash
#!/bin/bash
# healthcheck.sh
curl -f http://localhost:${SERVICE_PORT}/health || exit 1
```

### 3. 启动脚本

```bash
#!/bin/bash
# start.sh

# 等待区块链节点就绪
echo "Waiting for blockchain node..."
while ! nc -z localhost 8545; do
  sleep 1
done

# 等待Consul就绪
echo "Waiting for Consul..."
while ! nc -z localhost 8500; do
  sleep 1
done

# 启动服务
echo "Starting service..."
npm start
```

## 常见问题

### Q: 如何处理区块链连接失败？

```typescript
class BlockchainService {
  async connectWithRetry(maxRetries = 5) {
    for (let i = 0; i < maxRetries; i++) {
      try {
        await this.connect();
        return;
      } catch (error) {
        logger.warn(`Connection attempt ${i + 1} failed, retrying...`);
        await new Promise(resolve => setTimeout(resolve, 5000));
      }
    }
    throw new Error('Failed to connect to blockchain');
  }
}
```

### Q: 如何实现优雅关闭？

```typescript
const signals = ['SIGINT', 'SIGTERM', 'SIGQUIT'];

signals.forEach(signal => {
  process.on(signal, async () => {
    logger.info(`Received ${signal}, shutting down gracefully`);
    
    // 停止接收新请求
    server.close();
    
    // 注销Consul
    await consul.deregister();
    
    // 关闭数据库连接
    await database.close();
    
    process.exit(0);
  });
});
```

### Q: 如何处理Gas价格波动？

```typescript
async function getDynamicGasPrice() {
  const provider = getProvider();
  const feeData = await provider.getFeeData();
  
  // 根据网络情况调整
  const gasPrice = feeData.gasPrice;
  const adjustedPrice = gasPrice.mul(110).div(100); // 加10%缓冲
  
  return adjustedPrice;
}
```

## 性能优化建议

1. **使用连接池**
```typescript
const pool = new Pool({
  max: 10,
  min: 2,
  idleTimeoutMillis: 30000
});
```

2. **实现缓存**
```typescript
import NodeCache from 'node-cache';
const cache = new NodeCache({ stdTTL: 600 });
```

3. **批量处理**
```typescript
async function batchProcess(items: any[], batchSize = 10) {
  const results = [];
  for (let i = 0; i < items.length; i += batchSize) {
    const batch = items.slice(i, i + batchSize);
    const batchResults = await Promise.all(
      batch.map(item => processItem(item))
    );
    results.push(...batchResults);
  }
  return results;
}
```

## 监控集成

### Prometheus指标

```typescript
import { register, Counter, Histogram } from 'prom-client';

const httpRequestsTotal = new Counter({
  name: 'http_requests_total',
  help: 'Total number of HTTP requests',
  labelNames: ['method', 'route', 'status']
});

const httpRequestDuration = new Histogram({
  name: 'http_request_duration_seconds',
  help: 'Duration of HTTP requests in seconds',
  labelNames: ['method', 'route']
});

app.get('/metrics', async (req, res) => {
  res.set('Content-Type', register.contentType);
  res.end(await register.metrics());
});
```

## 服务通信模式

### 1. 直接调用模式
```typescript
// 服务A调用服务B
const response = await axios.get('http://service-b:8302/api/v1/resource');
```

### 2. 通过Gateway调用
```typescript
// 通过Gateway路由
const response = await axios.get('http://gateway:8000/api/v1/service-b/resource');
```

### 3. 事件驱动模式
```typescript
// 发布事件
eventBus.emit('resource.created', { id: '123', data: resource });

// 订阅事件
eventBus.on('resource.created', async (event) => {
  await processResourceCreated(event);
});
```

## 下一步行动

1. **创建服务**：基于模板创建你需要的服务
2. **实现功能**：按照规范实现具体业务逻辑
3. **编写测试**：确保代码质量
4. **本地测试**：在本地环境验证功能
5. **集成测试**：与其他服务联调
6. **部署上线**：按照部署流程发布服务

通过遵循这个实施指南，你可以快速创建符合 isA_Chain 架构规范的高质量微服务。