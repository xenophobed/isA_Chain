# isA_Chain — Agent-Native Token Economy PRD

> Product Requirements Document for the ISA token economy powering the isA platform.
> Last updated: 2026-03-22

## Vision

isA_Chain is the **settlement layer and economy backbone** for the entire isA platform. Every billable action — model inference, tool calls, compute hours, agent sessions — settles on-chain via the ISA token. The economy is designed **agent-first**: AI agents hold wallets, transact autonomously, and participate as first-class economic actors.

## Strategic Positioning

isA is an **end-to-end AI platform** — the only project with compute + models + tools + agents + apps under one token. This full-stack integration creates a closed-loop optimization flywheel that no point solution can replicate.

### Platform Stack (Value Chain)

```
Consumer Layer:  isA_Mate → isA_Trade, Creative, Marketing, Vibe
Agent Layer:     isA_Agent_SDK
Service Layer:   isA_MCP (tools) → isA_Model (inference)
Infra Layer:     isA_OS (compute) → isA_Data → isA_Cloud
Platform Layer:  isA_user (accounts/wallets) → isA_Console, isA_Docs
Showcase:        isA_Frame (reference agent-powered app)
Economy Layer:   isA_Chain (this project)
```

### Competitive Landscape

| Competitor | Scope | isA Advantage |
|-----------|-------|---------------|
| Render (RNDR) | GPU compute only | isA has full stack: compute + models + tools + agents + apps |
| Bittensor (TAO) | AI model subnets | isA has real platform services, not just competitive evaluation |
| OLAS/Autonolas | Agent services only | isA has the infrastructure agents run on |
| Virtuals Protocol | Agent tokenization | isA agents do real work, not just social personas |
| Akash (AKT) | Cloud compute only | isA integrates compute with AI-specific billing (per-token, per-call) |

## Token Economy Design

### Dual-Unit Model

| Unit | Purpose | Properties |
|------|---------|------------|
| **ISA Token** | Governance, staking, value accrual | 1B supply, deflationary via burns, scarce |
| **ISA Credits** | Agent-to-agent payments | Stable ($0.00001 each), high velocity, minted by burning ISA or depositing USDC |

### Why Dual-Unit

The **token velocity problem** kills single-token utility models — tokens change hands too fast, preventing value accrual. The dual-unit model solves this:
- ISA Credits handle high-frequency payments (velocity is fine for a stable unit)
- ISA Token captures value because you must **burn** it to mint Credits, **stake** it to provide services, and **hold** it to govern

### Token Distribution

| Allocation | % | Amount | Vesting |
|-----------|---|--------|---------|
| Community & Ecosystem | 40% | 400M | 4-year linear |
| Team & Contributors | 20% | 200M | 4-year, 1-year cliff |
| Treasury | 15% | 150M | DAO-governed |
| Provider Incentives | 15% | 150M | 10-year emission curve |
| Early Supporters | 10% | 100M | 2-year, 6-month cliff |

### Chain Parameters

| Parameter | Value |
|-----------|-------|
| Chain ID | 15489 (mainnet), 15490 (testnet) |
| Block time | 3 seconds |
| Max gas per block | 30,000,000 |
| Total supply | 1,000,000,000 ISA |
| Protocol fee | 2.5% |
| Min validator stake | 32,000 ISA |
| Min provider stake | 1,000 ISA |
| Credit rate | 1 Credit = $0.00001 USD |

## Core Mechanisms

### 1. Burn-Mint Settlement

Users burn ISA to consume services. Providers receive minted ISA minus protocol fee.

```
User burns 100 ISA → Provider receives 97.5 ISA (minted) → Treasury receives 2.5 ISA
```

### 2. Subnet Economy

Each service layer operates as a semi-autonomous subnet with its own staking, quality scoring, and emission allocation.

| Subnet | Service | Emission Weight |
|--------|---------|----------------|
| Model | isA_Model (inference) | 25% |
| Compute | isA_OS (VMs, GPUs) | 20% |
| Agent | isA_Agent_SDK (sessions) | 20% |
| Tool | isA_MCP (API calls) | 15% |
| Data | isA_Data (queries) | 10% |
| App | isA_Mate, Trade, Creative | 10% |

### 3. Agent Wallets

Every agent (not just users) has an on-chain wallet with autonomous spending authority.

```
Human sets budget → Agent Wallet enforces limits → Agent transacts autonomously
  - max_per_transaction
  - max_daily
  - max_monthly
  - allowed_subnets
```

### 4. Payment Channels

High-frequency agent-to-agent micropayments via state channels. Thousands of off-chain updates per on-chain settlement.

### 5. Agent Registry & Reputation

On-chain registry of all agents with capabilities, pricing, and reputation scores. Agents discover and trust each other through on-chain records.

### 6. Governance (veISA)

Lock ISA for 1-4 years → receive veISA (voting power). Longer lock = more weight. Vote on subnet emission weights, protocol fees, treasury spending.

## Integration Points (Existing Infrastructure)

| isA Service | Existing Hook | Chain Integration |
|-------------|--------------|-------------------|
| isA_Cloud | NATS billing pipeline (UsageEvent → BillingCalculated → TokensDeducted) | Settlement bridge subscribes to NATS, batches to chain |
| isA_user wallet_service | `blockchain_address`, `blockchain_tx_hash`, `on_chain_balance` fields | Direct RPC calls to chain for balance sync |
| isA_user vault_service | `BLOCKCHAIN_KEY` secret type | Stores agent/user private keys |
| isA_user credit_service | `1 Credit = $0.00001` | Maps to ISA Credits on-chain |
| isA_Model | BillingMiddleware (402 on insufficient), NATS usage events | Pre-inference Credit check, post-inference settlement |
| isA_OS | QuotaEnforcer, tiered pricing, per-minute billing | Compute subnet settlement |
| isA_MCP | Usage recording (call_count, success_rate), Redis rate limiting | Tool subnet settlement |
| isA_Agent_SDK | Node timing, tool execution counts, observability metrics | Agent wallet integration in AgentStack |
| isA_App_SDK | WalletService client (deposit, withdraw, consume, transfer) | On-chain wallet operations |

## Phased Delivery

### Phase 1: Foundation (Months 1-3)
ISA token, staking, storage persistence, wallet bridge, usage settlement bridge.
**Deliverable**: Single-node chain with token payments for inference.

### Phase 2: Subnet Economy (Months 4-6)
Subnet registry, per-subnet staking, emission controller, quality oracle, compute marketplace activation.
**Deliverable**: Multi-service economy with provider competition.

### Phase 3: Agent Economy (Months 7-10)
ISA Credits, agent wallet factory, payment channels, agent registry, budget delegation, isA_Mate integration.
**Deliverable**: Full agent-to-agent economy.

### Phase 4: Governance & Scale (Months 11-14)
veISA governance, cross-chain bridge, agent tokenization, developer incentives, P2P networking.
**Deliverable**: Production economy with decentralized governance.

## Out of Scope (for now)

- Smart contract VM (EVM/WASM execution engine) — future phase
- Cross-chain atomic swaps — future phase
- Fiat on/off ramp (Stripe integration exists in isA_user, bridge later)
- Mobile wallet app — use isA_App_SDK instead
- Layer 2 rollup — evaluate after mainnet stability

## Success Metrics

| Metric | Phase 1 Target | Phase 4 Target |
|--------|---------------|----------------|
| On-chain transactions/day | 1,000 | 100,000 |
| Active agent wallets | 10 | 1,000 |
| ISA burned/month | 10,000 | 1,000,000 |
| Provider stakers | 5 | 100 |
| Settlement latency (NATS → chain) | < 60s | < 5s |
| Unique users with on-chain balance | 50 | 5,000 |
