# Testing

## Automated Test Run (2026-02-09 16:53:59)

- Command: `npm test`
- Status: `timeout`
- Exit: `None`
- Duration: `120.02s`
- Stdout (tail):
```
test account::tests::test_balance_operations ... ok
test account::tests::test_account_creation ... ok
test account::tests::test_delegation ... ok
test account::tests::test_validator_creation ... ok
test block::tests::test_block_succession ... ok
test block::tests::test_block_creation_and_verification ... ok
test block::tests::test_genesis_block_creation ... ok
test compute_market::tests::test_accept_job_already_accepted ... ok
test compute_market::tests::test_accept_job_price_too_high ... ok
test compute_market::tests::test_cancel_job_by_user ... ok
test compute_market::tests::test_accept_job_wrong_provider ... ok
test compute_market::tests::test_cancel_job_unauthorized ... ok
test compute_market::tests::test_cancel_running_job_fails ... ok
test compute_market::tests::test_capacity_deduction_on_accept ... ok
test compute_market::tests::test_capacity_restoration_on_cancel ... ok
test compute_market::tests::test_cancel_job_by_provider ... ok
test compute_market::tests::test_create_job_inactive_provider ... ok
test compute_market::tests::test_create_job_duplicate_id ... ok
test compute_market::tests::test_create_job_nonexistent_provider ... ok
test compute_market::tests::test_create_job_without_specific_provider ... ok
test compute_market::tests::test_escrow_tracking ... ok
test compute_market::tests::test_find_matching_providers ... ok
test compute_market::tests::test_insufficient_capacity_rejection ... ok
test compute_market::tests::test_create_job_unsupported_resource_type ... ok
test compute_market::tests::test_job_lifecycle ... ok
test compute_market::tests::test_list_providers_by_resource ... ok
test compute_market::tests::test_list_user_jobs ... ok
test compute_market::tests::test_market_stats_accuracy ... ok
test compute_market::tests::test_open_dispute_on_running_job ... ok
test compute_market::tests::test_provider_exit_success ... ok
test compute_market::tests::test_open_dispute_unauthorized ... ok
test compute_market::tests::test_provider_exit_with_active_job ... ok
test compute_market::tests::test_provider_registration ... ok
test compute_market::tests::test_provider_registration_insufficient_stake ... ok
test compute_market::tests::test_provider_update_nonexistent ... ok
test compute_market::tests::test_provider_update ... ok
test compute_market::tests::test_provider_reputation_update ... ok
test compute_market::tests::test_resolve_dispute_already_resolved ... ok
test compute_market::tests::test_settle_job_not_running ... ok
test compute_market::tests::test_settle_job_protocol_fee ... ok
test compute_market::tests::test_resolve_dispute_with_slashing ... ok
test compute_market::tests::test_start_job_not_matched ... ok
test compute_market::tests::test_start_job_wrong_provider ... ok
test crypto::tests::test_hashing ... ok
test crypto::tests::test_address_derivation ... ok
test crypto::tests::test_key_generation ... ok
test error::tests::test_api_error ... ok
test error::tests::test_error_codes ... ok
test transaction::tests::test_invalid_stake_amount ... ok
test transaction::tests::test_transaction_hash_consistency ... ok
test types::tests::test_address_from_public_key ... ok
test types::tests::test_hash_creation ... ok
test types::tests::test_signature_serialization ... ok
test types::tests::test_merkle_root ... ok
test transaction::tests::test_transaction_creation_and_signing ... ok

test result: ok. 55 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


> isa-chain@0.1.0 test:contracts
> cd contracts && npm run test


> isa-chain@0.1.0 test
> npm run test:core && npm run test:contracts && npm run test:dapp


> isa-chain@0.1.0 test:core
> cargo test --manifest-path=core/blockchain/Cargo.toml

```
- Stderr (tail):
```
3 | use crate::error::*;
  |     ^^^^^^^^^^^^^^^

warning: unused import: `crate::error::*`
 --> core/blockchain/src/state.rs:4:5
  |
4 | use crate::error::*;
  |     ^^^^^^^^^^^^^^^

warning: unused import: `Amount`
 --> core/blockchain/src/rpc/handlers.rs:7:29
  |
7 | use crate::types::{Address, Amount, ChainId};
  |                             ^^^^^^

warning: unused imports: `Address` and `Hash`
 --> core/blockchain/src/rpc/types.rs:2:20
  |
2 | use crate::types::{Hash, Address};
  |                    ^^^^  ^^^^^^^

warning: unused import: `crate::error::BlockchainError`
 --> core/blockchain/src/compute_market.rs:6:5
  |
6 | use crate::error::BlockchainError;
  |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

warning: unused imports: `TransactionData` and `TransactionError`
 --> core/blockchain/src/compute_market.rs:7:26
  |
7 | use crate::transaction::{TransactionData, TransactionError};
  |                          ^^^^^^^^^^^^^^^  ^^^^^^^^^^^^^^^^

warning: ambiguous glob re-exports
  --> core/blockchain/src/lib.rs:19:9
   |
19 | pub use transaction::*;
   |         ^^^^^^^^^^^^^^ the name `ValidatorDescription` in the type namespace is first re-exported here
20 | pub use account::*;
   |         ---------- but the name `ValidatorDescription` in the type namespace is also re-exported here
   |
   = note: `#[warn(ambiguous_glob_reexports)]` on by default

warning: unused variable: `params`
   --> core/blockchain/src/rpc/handlers.rs:265:59
    |
265 |     async fn handle_get_block_by_number(&self, id: Value, params: Value) -> JsonRpcResponse {
    |                                                           ^^^^^^ help: if this is intentional, prefix it with an underscore: `_params`
    |
    = note: `#[warn(unused_variables)]` on by default

warning: unused variable: `chain_id`
   --> core/blockchain/src/block.rs:279:20
    |
279 |     pub fn genesis(chain_id: ChainId) -> Self {
    |                    ^^^^^^^^ help: if this is intentional, prefix it with an underscore: `_chain_id`

warning: field `height` is never read
  --> core/blockchain/src/state.rs:16:5
   |
8  | pub struct WorldState {
   |            ---------- field in this struct
...
16 |     height: BlockHeight,
   |     ^^^^^^
   |
   = note: `#[warn(dead_code)]` on by default

warning: `isa-chain-core` (lib) generated 13 warnings (run `cargo fix --lib -p isa-chain-core` to apply 9 suggestions)
warning: unused import: `TransactionType`
   --> core/blockchain/src/block.rs:354:47
    |
354 |     use crate::transaction::{TransactionData, TransactionType};
    |                                               ^^^^^^^^^^^^^^^

warning: `isa-chain-core` (lib test) generated 14 warnings (13 duplicates) (run `cargo fix --lib -p isa-chain-core --tests` to apply 1 suggestion)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.38s
     Running unittests src/lib.rs (target/debug/deps/isa_chain_core-3bddc436eb9bcb4b)
     Running unittests src/bin/node.rs (target/debug/deps/isa_chain_node-f7add9c482a32194)
   Doc-tests isa_chain_core
```

## References

- [PROJECT.md](./PROJECT.md)
- [README.md](./README.md)
- [services-readme.md](./services-readme.md)
- [services/defi-api/README.md](./services/defi-api/README.md)
