# 🔧 isA_Chain 服务管理指南

## 概述

本指南详细介绍如何使用 `scripts/start-services.sh` 脚本管理 isA_Chain 生态系统的各个服务组件。

## 🏗️ 架构概览

### 区块链层
- **Hardhat**: 以太坊兼容的开发测试链 (端口: 8545)
- **isA_Chain**: 自定义多链兼容区块链 (端口: 9944)

### 微服务层
- **DeFi Service**: DeFi协议API服务 (端口: 8313)
- **NFT Service**: NFT平台API服务 (端口: 8312)
- **Tools Service**: 工具和分析API服务 (端口: 8315)

### 服务发现
- **Consul**: 服务注册和发现 (端口: 8500)

## 🚀 快速开始

### 启动所有服务
```bash
./scripts/start-services.sh start
```

系统会提示选择区块链:
1. Hardhat (测试/开发环境)
2. isA_Chain (自定义区块链)
3. 两者都启动

### 查看服务状态
```bash
./scripts/start-services.sh status
```

### 停止服务
```bash
# 停止微服务，保留区块链
./scripts/start-services.sh stop

# 停止所有服务包括区块链
./scripts/start-services.sh stop-all
```

## 📋 详细命令说明

### 1. start - 启动服务
```bash
./scripts/start-services.sh start
```

**功能:**
- 检查必要依赖 (Node.js, npm, Hardhat)
- 提供区块链选择界面
- 启动选定的区块链
- 依次启动所有微服务
- 显示最终状态和访问URL

**输出示例:**
```
╔══════════════════════════════════════════════════════════╗
║          isA_Chain Microservices Launcher                ║
╚══════════════════════════════════════════════════════════╝

✓ isA_Chain started (PID: 32492)
✓ defi-api started successfully (PID: 32495)
✓ nft-api started successfully (PID: 56260)
✓ tools-api started successfully (PID: 56514)

Services are ready!
```

### 2. status - 查看状态
```bash
./scripts/start-services.sh status
```

**显示信息:**
- 各个区块链的运行状态和PID
- Consul服务发现状态
- 所有微服务的运行状态和端口

### 3. stop - 停止微服务
```bash
./scripts/start-services.sh stop
```

**功能:**
- 停止所有微服务进程
- 清理PID文件
- 保留区块链继续运行

### 4. stop-all - 停止所有服务
```bash
./scripts/start-services.sh stop-all
```

**功能:**
- 停止所有微服务
- 停止所有区块链
- 完全清理环境

### 5. restart - 重启微服务
```bash
./scripts/start-services.sh restart
```

### 6. restart-all - 重启所有服务
```bash
./scripts/start-services.sh restart-all
```

### 7. logs - 查看日志
```bash
./scripts/start-services.sh logs
```

**功能:**
- 实时追踪所有微服务的日志输出
- 使用 `Ctrl+C` 停止日志追踪

### 8. blockchain - 区块链管理
```bash
# 查看区块链状态
./scripts/start-services.sh blockchain status

# 启动区块链
./scripts/start-services.sh blockchain start

# 停止区块链
./scripts/start-services.sh blockchain stop

# 查看区块链日志
./scripts/start-services.sh blockchain logs
```

## 🔍 服务端点

### 通过网关访问 (推荐)
- DeFi Service: `http://localhost:8000/api/v1/defi-service/`
- NFT Service: `http://localhost:8000/api/v1/nft-service/`
- Tools Service: `http://localhost:8000/api/v1/tools-service/`

### 直接访问 (开发调试)
- DeFi API: `http://localhost:8313/health`
- NFT API: `http://localhost:8312/health`
- Tools API: `http://localhost:8315/health`

### 区块链RPC
- Hardhat: `http://localhost:8545`
- isA_Chain: `http://localhost:9944`
- isA_Chain WebSocket: `ws://localhost:9945`

## 🐛 故障排除

### 常见问题

#### 1. 端口占用
**现象:** 服务启动失败，提示端口已被占用
```
✗ Port 8312 is already in use
```

**解决方法:**
```bash
# 查找占用端口的进程
lsof -i :8312

# 终止进程 (替换PID)
kill <PID>
```

#### 2. 依赖缺失
**现象:** 服务启动时报模块找不到
```
Cannot find module './services/ipfs.service'
```

**解决方法:**
```bash
# 重新安装依赖
cd services/nft-api
npm install
```

#### 3. 区块链编译失败
**现象:** isA_Chain启动失败
```
isA_Chain binary not found. Please compile first
```

**解决方法:**
```bash
cd core/blockchain
cargo build --bin isa-chain-node
```

#### 4. Consul未运行
**现象:** 警告信息显示Consul未运行
```
⚠ Warning: Consul is not running on localhost:8500
```

**解决方法:**
```bash
# 使用Docker启动Consul
docker run -d --name=consul -p 8500:8500 consul:latest agent -server -ui -node=server-1 -bootstrap-expect=1 -client=0.0.0.0
```

### 日志文件位置
- 微服务日志: `services/[service-name]/service.log`
- Hardhat日志: `hardhat.log`
- isA_Chain日志: `isachain.log`

### 调试模式
```bash
# 设置调试环境变量
export DEBUG=isa-chain:*

# 启动服务查看详细日志
./scripts/start-services.sh start
```

## 🔧 配置管理

### 环境变量
每个微服务都有 `.env` 配置文件:
- `services/defi-api/.env`
- `services/nft-api/.env`  
- `services/tools-api/.env`

### 默认端口配置
脚本中的端口配置在 `start-services.sh` 顶部:
```bash
services_list="defi-api:8313 nft-api:8312 tools-api:8315"
HARDHAT_PORT=8545
ISA_CHAIN_PORT=9944
ISA_CHAIN_P2P_PORT=9945
```

## 📊 监控和维护

### 健康检查
所有服务都提供健康检查端点:
```bash
curl http://localhost:8313/health  # DeFi Service
curl http://localhost:8312/health  # NFT Service  
curl http://localhost:8315/health  # Tools Service
```

### 性能监控
通过Consul Dashboard监控服务:
```
http://localhost:8500/ui/
```

### 定期维护
建议定期执行:
```bash
# 重启服务释放内存
./scripts/start-services.sh restart-all

# 清理日志文件
find . -name "*.log" -size +100M -delete
```

## 🚀 生产环境部署

### Docker部署
```bash
# 构建镜像
docker-compose build

# 启动生产环境
docker-compose up -d
```

### 系统服务配置
创建systemd服务文件实现开机自启:
```bash
sudo cp scripts/isa-chain.service /etc/systemd/system/
sudo systemctl enable isa-chain
sudo systemctl start isa-chain
```

---
*最后更新: 2024年9月25日*