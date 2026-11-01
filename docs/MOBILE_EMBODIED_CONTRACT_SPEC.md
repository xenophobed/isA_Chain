# isA_Chain ↔ isA Mobile & 具身智能 (Embodied) 核心契约规范

版本: v1.0.0
生效环境: isA_Chain TestNet (15490) / MainNet (15489)
状态: Draft / In Review

---

## 一、 移动端硬件去中心化身份 (Mobile DID Specification)

### 1.1 规范背景
在 `isa_mobile` 中，用户私钥存放在移动端硬件安全芯片（iOS Secure Enclave / Android StrongBox KeyStore）。
身份标识统一采用 W3C DID Core 1.0 规范：
- **DID 格式**: `did:isa:<20-byte-hex-address>`
- **派生地址**: `0x` + `sha256(publicKeyHex)[-40:]` (例如: `0x71c062837bc2c0612140a33a39e80e1a8a25c4f9`)

### 1.2 链上 DID 注册表结构 (DIDRegistry)
链上维护每个 DID 的硬件凭据状态，防止中间人伪造与重放：

```rust
pub struct DIDDocumentRecord {
    pub did: String,                       // "did:isa:0x..."
    pub owner_address: Address,            // 20-byte 链上地址
    pub public_key_hex: String,            // 硬件导出的公钥 Hex
    pub hardware_backed: bool,             // 是否经由 Secure Enclave / StrongBox 鉴真
    pub status: DIDStatus,                 // Active | Suspended | Revoked
    pub registered_at: BlockHeight,        // 注册所在区块高度
    pub updated_at: BlockHeight,           // 更新所在区块高度
    pub delegations: Vec<AgentDelegation>, // 授权的智能体委托凭据列表
}

pub struct AgentDelegation {
    pub agent_id: Hash,                    // 目标 isA Agent 标识
    pub max_credits_allowance: Amount,     // 最大调用/消费信用额度
    pub expires_at: Timestamp,             // 委托到期时间戳 (ms)
    pub granted_actions: Vec<String>,      // 允许代理执行的动作清单
}
```

### 1.3 对应 RPC 接口
- `did_register(did_doc, signature)`: 将移动端硬件 DID 写入链上。
- `did_resolve(did)`: 解析指定 DID 的公钥、硬件背书状态及当前委托。
- `did_grantDelegation(agent_id, allowance, expires_at, signature)`: 移动端签署委托授权。

---

## 二、 具身物理动作排他租约与回执存证 (Embodied Physical Lease & Receipt)

### 2.1 规范背景
结合 `isA_Docs/embodied/cloud-edge-safety` 与 `ral-protocol`：
- **云端与智能体**：发起具备物理影响的操作（机械臂 ORCA / 仿生手 RP Hand）；
- **看门狗安全约束**：具备 60 秒限时排他租约（Lease），严禁任何静默重试；
- **边缘硬件层 (RAL)**：闭环检验稳态（容差收敛、传感器采样 15ms），反馈 `completion_verified`。

### 2.2 动作租约防重放契约 (ActionLeaseRecord)

```rust
pub struct ActionLeaseRecord {
    pub lease_id: Hash,                    // 唯一租约哈希: sha256(device_id + nonce + target_action)
    pub device_id: String,                 // 物理硬件唯一标识 (如 "orca-arm-01", "rp-hand-left")
    pub agent_id: Hash,                    // 发起动作的自主智能体
    pub target_component: String,          // 目标致动器: "hand" | "manipulator" | "base"
    pub action_name: String,               // 动作名称 (如 "reach_and_grasp", "joint_trajectory")
    pub action_params_hash: Hash,          // 动作参数哈希 (位置/速度/力矩限值)
    pub nonce: u64,                        // 设备级严格递增防重放 Nonce
    pub lease_duration_secs: u32,          // 租约有效时长 (默认 60s)
    pub status: LeaseStatus,               // Granted | InProgress | Executed | Expired | EstopAborted
    pub granted_at_block: BlockHeight,     // 授予高度
    pub granted_at_time: Timestamp,        // 授予时间戳 (ms)
    pub expires_at_time: Timestamp,        // 绝对过期时间戳 (ms)
}

pub struct ActionCompletionProof {
    pub lease_id: Hash,                    // 关联的租约 ID
    pub receipt_hash: Hash,                // 边缘 RAL 传感器闭环特征哈希
    pub encoder_positions: Vec<i64>,       // 终态物理编码器位置 (原生规范单位)
    pub steady_duration_ms: u32,           // 稳态持续时间 (需 >= 15ms)
    pub completion_verified: bool,         // RAL 物理闭环确认标志
    pub completed_at_time: Timestamp,      // 完成时间戳
    pub signature: String,                 // 边缘硬件节点设备私钥签名
}
```

### 2.3 对应 RPC 接口
- `embodied_requestActionLease(device_id, action_payload, nonce, agent_sig)`: 申请排他动作租约。
- `embodied_submitCompletionProof(lease_id, proof, hardware_sig)`: 提交物理闭环完成存证。
- `embodied_verifyActionStatus(lease_id)`: 查询动作存证与上链结果。

---

## 三、 Agent 微结算与 MCP 映射 (Agent Economy & MCP Integration)

智能体通过标准 MCP 工具交互：
1. `isa_chain_balance`: 查询当前账户或 Agent 代币与信用余额；
2. `isa_chain_request_physical_lease`: 为具身硬件执行请求 60s 动作排他租约；
3. `isa_chain_settle_service`: 跨 Agent 或服务调用结束后的链上计费结算。
