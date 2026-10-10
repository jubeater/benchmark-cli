# benchmark-cli

Validate eval-schema v2 results, with one JSON record per line. Only schema
version `2.0` is supported.

```sh
cargo run -- --input data/v2/valid.jsonl
```

For your own results, pass `--input path/to/results.jsonl` (or `-i`).

The CLI parses each line, checks records with the v2 validator, and prints a
summary:

```text
Eval-schema v2 validation
Accepted records: 4
Rejected records: 0
Superseded records: 1
```

An unsuccessful measurement (for example `status: "oom"`) can still be an
accepted record. The counts describe record validity, not measurement success.
A fully valid file exits with code 0. Each rejected record is reported on
stderr as `line N: <field>: <reason>` (for unparseable JSON the field is
`<json>`); any rejection exits with code 1, while a missing input file exits
with code 2.

Records are also checked for accidental duplicate measurements in file order.
The measurement key hashes the run ID, metric, canonical config hash, seed,
dataset fingerprint, and evaluator fingerprint. Object key order does not
affect identity; absent and null config keys, and integer and float values,
remain distinct. A duplicate is rejected with the original line number:

```text
line 2: measurement_key: duplicate measurement (first seen on line 1)
```

A correction's `supersedes_record_id` must identify a record accepted earlier in
the file with the same measurement key. Dangling references, corrections before
their target, wrong-target corrections, and corrections of unseen measurements
are rejected on the `supersedes_record_id` field. Invalid records do not reserve
measurement keys or become valid correction targets.

Records are resolved in file order in one pass. Corrections replace the effective
record for their measurement; chains resolve to the last accepted correction.
The resolver exposes exactly one effective record per measurement key for future
aggregation. Accepted counts include superseded records; the superseded count
reports how many accepted records were displaced by corrections.

When records are aggregated across seeds, effective records are grouped by
`(run_id, metric, unit, config_hash, dataset_fingerprint, evaluator_fingerprint)` in
stable sorted order. Records containing an `aggregate` key in their `config`
(such as `"aggregate": "median_of_3"`) are excluded. The CLI computes `n_attempted`
across all statuses, `n_ok`, and `median` / `min` / `max` over `status: ok` values.
For even counts, the median is the mean of the two middle values. When `n_ok = 0`,
statistics are displayed as `N/A`. A warning is printed for any group with fewer than
3 `ok` seeds (enforcing the three-seed rule):

```text
run_id                         | metric         | unit           | config_hash                                                      | dataset_fingerprint | evaluator_fingerprint | n_attempted | n_ok | median | min | max
-------------------------------+----------------+----------------+------------------------------------------------------------------+---------------------+-----------------------+-------------+------+--------+-----+----
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | 297b1e2373ca8181a16df4aaa7b57e6f27a232e43aa658d798d97e9246dbe749 | null                | null                  |           3 |    3 |    320 | 300 | 350
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | db8be9fbe8f84ff4d2e61f9cf1bfa233deaabd47ecca04dbc7d065c66a175395 | null                | null                  |           3 |    3 |    102 | 100 | 105
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf | null                | null                  |           3 |    2 |    205 | 200 | 210

warning: group (run_id: 2026-10-09T14:00:00Z-sweep-001, metric: tokens_per_sec, config_hash: fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf) has fewer than 3 ok seeds (n_ok = 2)
```

The fixture in `data/v2/valid.jsonl` covers absent, populated, and null optional
blocks. The fixture in `data/v2/invalid.jsonl` covers one defect per line
(missing field, wrong type, bad enum, malformed block, bad JSON). The identity fixture
covers an accidental duplicate, a valid correction, a dangling correction, and a correction
that targets a different measurement. The sweep fixture in `data/v2/sweep.jsonl` covers
a batch size sweep across seeds with an OOM measurement, an aggregate record, and hand-computed
medians. Expected outcomes are documented in `data/v2/README`.

Run the checks with:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

## Known limitations

Not handled, by design — none occur if records follow the schema:

- Duplicate keys inside one JSON line: last value wins.
- Integers beyond 64-bit are rounded to f64; distinct values can hash the same.
- Two corrections of the same record (fork): the later one in the file wins.
- Unknown keys inside blocks are accepted.
- Model/env consistency within a group is not checked.
- LLM-run rules and judge-calibration rules are not enforced.
