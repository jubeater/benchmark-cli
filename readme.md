##
How to run:

```rust
cargo run -- --input [AI_log_jsonl_path] --pricing [model_pricing_json_path]
```
Example usage:

```rust
cargo run -- --input data/sample.jsonl --pricing data/pricing.json
```

Example output:

```text
AI Log Analysis
--------------------------

Requests
  Total:                 8
  Successful:            4
  Failed:                4
  Error rate:            50.00%
--------------------------
Data Quality
  Invalid records:       1
--------------------------

Latency
  Min:                   150 ms
  Avg:                   166.00 ms
  P50:                   150 ms
  P95:                   182 ms
  P99:                   182 ms
  Max:                   182 ms
--------------------------

Token Usage
  Input:                 8000
  Output:                2200
  Total:                 10200
--------------------------

Cost
  Known-priced cost:    $0.02
  Unpriced requests:     4
⚠ Cost is incomplete
```
