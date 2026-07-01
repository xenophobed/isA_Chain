# ADR-0001: Metrics-only observability for isA_Chain (revisit full observability later)

**Date**: 2026-06-20
**Status**: Accepted

## Context

`isA_Chain` is a Rust blockchain node. Issue #88 asked us to make the
observability scope **explicit and intentional** rather than accidental. A scan
of the current implementation found a partial, inconsistent picture:

- **Metrics** — a hand-rolled Prometheus text-exposition endpoint at `/metrics`
  (`core/blockchain/src/metrics.rs`, served from `core/blockchain/src/rpc/server.rs`),
  exposing 6 series: `isa_chain_height`, `isa_chain_mempool_size`,
  `isa_chain_account_count`, `isa_chain_blocks_produced_total`,
  `isa_chain_transactions_processed_total`, `isa_chain_rpc_requests_total`.
  There is no `prometheus` crate dependency — the exposition format is built by hand.
- **Tracing** — **absent.** No `opentelemetry` / `tracing-opentelemetry` / OTLP
  exporter anywhere in the codebase.
- **Logging** — `tracing` + `tracing-subscriber` configured in **plain text**
  (`core/blockchain/src/bin/node.rs`), driven by `RUST_LOG` (default `isa_chain=info`).
  Not structured/JSON, so it is not aggregation-friendly.
- **Kubernetes** — `deployment/helm/values.yaml` declares
  `observability.tempoHost`, `tempoPort`, `lokiUrl`, and `isaEnv`, but **none are
  wired** to the running node. There is **no ServiceMonitor and no Prometheus
  scrape annotation**, so even the existing `/metrics` endpoint is not actually
  collected in the cluster.

The forces at play: the node is **not yet in production** (the implementation is
still local, pre-deployment), a full OpenTelemetry traces + Loki log-shipping
stack is a meaningful lift for a single Rust service, and the platform may later
define a cross-service observability standard that `isA_Chain` should conform to
rather than inventing its own now.

## Decision

Adopt **metrics-only observability for now**, and explicitly **defer** full
observability (distributed traces + log shipping) until a platform-wide standard
is established.

Concretely, for this phase:

- The Prometheus `/metrics` endpoint is the **single intended observability
  surface** for `isA_Chain`.
- OpenTelemetry traces and Loki/Tempo log shipping are **out of scope** for now,
  by decision — not by omission.
- The dangling `observability.tempoHost` / `tempoPort` / `lokiUrl` Helm values
  are documented as **aspirational / not-yet-wired**; they will be removed or
  wired as part of the deferred follow-up, not left to imply working behavior.

No code or Helm changes are made by this ADR itself — it records the decision and
satisfies #88's acceptance criteria ("metrics-only vs full observability is
documented and intentional"). The concrete metrics-only hardening (K8s scraping +
structured logs) and the eventual revisit are tracked as a follow-up task.

## Alternatives Considered

1. **Adopt full observability now (OTEL traces + Loki log shipping)** — rejected
   for this phase: significant dependency and bootstrap work (`opentelemetry`,
   `tracing-opentelemetry`, OTLP exporter, collector wiring) for a service that
   is not yet in production and has no consumers of traces today. Premature given
   no platform standard exists to conform to.
2. **Leave it undocumented (status quo)** — rejected: that is exactly the
   accidental, ambiguous state #88 was filed to fix. The Helm file's unwired
   Tempo/Loki references actively mislead readers into thinking log/trace
   shipping works.
3. **Remove the metrics endpoint entirely** — rejected: the `/metrics` endpoint
   is cheap, already implemented, and provides real operational value (chain
   height, mempool depth, throughput) once it is actually scraped.

## Consequences

### Positive
- Observability scope is now **explicit and intentional**, closing #88.
- Minimal ongoing maintenance burden — no trace/exporter infrastructure to run.
- Leaves room to adopt a future platform observability standard rather than
  committing to a bespoke OTEL setup now.

### Negative
- No distributed tracing — cross-service request flows (e.g. settlement bridge,
  RPC → consensus) cannot be traced end-to-end.
- Logs remain plain-text and node-local; no centralized log aggregation.
- The `/metrics` endpoint provides value only once K8s scraping is wired (tracked
  in the follow-up); until then, metrics are reachable but not collected.

### Risks
- If `isA_Chain` enters production before the follow-up lands, operators have
  metrics exposed but **not collected**, and only plain-text local logs — limited
  incident-debugging visibility. The follow-up should be done before production.
- The decision must be **revisited** when a platform observability standard
  emerges; if that signal is missed, `isA_Chain` could drift out of alignment.

## References
- Issue #88 — "Clarify metrics-only vs full observability architecture"
- `core/blockchain/src/metrics.rs`, `core/blockchain/src/rpc/server.rs`
- `core/blockchain/src/bin/node.rs`
- `deployment/helm/values.yaml`
- Follow-up task: metrics-only hardening (K8s scraping + structured logs)
