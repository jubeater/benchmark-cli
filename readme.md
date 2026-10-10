# benchmark-cli

Validate eval-schema v2 results, with one JSON record per line. Only schema
version `2.0` is supported.

```sh
cargo run -- --input data/v2/valid.jsonl
```

For your own results, pass `--input path/to/results.jsonl` (or `-i`).

Empty and whitespace-only lines are skipped. They do not count as accepted or
rejected records; reported line numbers still refer to physical lines in the file.

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

Each accepted record must have a unique `record_id`, including corrections.
Reusing an accepted ID is rejected with its first accepted line number:

```text
line 2: record_id: duplicate record_id (first seen on line 1)
```

A correction's `supersedes_record_id` must identify a record accepted earlier in
the file with the same measurement key. Dangling references, corrections before
their target, wrong-target corrections, and corrections of unseen measurements
are rejected on the `supersedes_record_id` field. Invalid records do not reserve
measurement keys or record IDs, or become valid correction targets.

Records are resolved in file order in one pass. Corrections replace the effective
record for their measurement; chains resolve to the last accepted correction.
The resolver supplies exactly one effective record per measurement key for
aggregation. A rejected correction leaves its target's effective record unchanged.
Accepted counts include superseded records; the superseded count
reports how many accepted records were displaced by corrections.

When records are aggregated across seeds, effective records are grouped by
`(run_id, metric, unit, config_hash, dataset_fingerprint, evaluator_fingerprint)` in
stable sorted order. Records containing an `aggregate` key in their `config`
(such as `"aggregate": "median_of_3"`) are excluded. The CLI computes `n_attempted`
across all statuses, `n_ok`, and `median` / `min` / `max` over `status: ok` values.
For even counts, the median is the mean of the two middle values. When `n_ok = 0`,
statistics are displayed as `N/A`. A warning is printed for any group with fewer than
3 `ok` seeds (enforcing the three-seed rule). Text tables and warnings display
the first 12 characters of each hash; `null` fingerprints stay `null`. For example,
`--input data/v2/sweep.jsonl` produces:

```text
run_id                         | metric         | unit           | config_hash  | dataset_fingerprint | evaluator_fingerprint | n_attempted | n_ok | median | min | max
-------------------------------+----------------+----------------+--------------+---------------------+-----------------------+-------------+------+--------+-----+----
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | 297b1e2373ca | null                | null                  |           3 |    3 |    320 | 300 | 350
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | db8be9fbe8f8 | null                | null                  |           3 |    3 |    102 | 100 | 105
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | fc046d91d2e2 | null                | null                  |           3 |    2 |    205 | 200 | 210

warning: group (run_id: 2026-10-09T14:00:00Z-sweep-001, metric: tokens_per_sec, config_hash: fc046d91d2e2) has fewer than 3 ok seeds (n_ok = 2)
```

If any records are rejected, the CLI still shows statistics from accepted,
effective records. Rejected records never enter aggregation. The result is
incomplete and exits with code 1, with each rejection reported on stderr. Text
output shows a notice above the group table (for example, with one rejection):

```text
Incomplete result: statistics exclude rejected records (count: 1).
```

Passing `--group-by <key>[,<key>...]` shows the chosen config keys as columns in
the group table so sweeps read naturally (for example `--group-by batch_size`).
This is presentation only: grouping itself is unchanged and still uses the full
config hash, so `--group-by` never merges different configurations. Multiple
keys can be passed as a comma-separated list or by repeating the flag. If a
group's config lacks a requested key, it renders as `absent`, distinct from an
explicit `null`. In text output, rows sort by the requested keys in flag order:
numbers numerically, then strings lexically, then booleans, arrays, and objects
(the latter two by their canonical JSON text), followed by null and absent values.
Ties use the full group key. Without `--group-by`, the original stable group-key
order is preserved. The same sweep with `--group-by batch_size` produces:

```text
run_id                         | metric         | unit           | config_hash  | dataset_fingerprint | evaluator_fingerprint | batch_size | n_attempted | n_ok | median | min | max
-------------------------------+----------------+----------------+--------------+---------------------+-----------------------+------------+-------------+------+--------+-----+----
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | db8be9fbe8f8 | null                | null                  | 1          |           3 |    3 |    102 | 100 | 105
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | fc046d91d2e2 | null                | null                  | 8          |           3 |    2 |    205 | 200 | 210
2026-10-09T14:00:00Z-sweep-001 | tokens_per_sec | tokens_per_sec | 297b1e2373ca | null                | null                  | 32         |           3 |    3 |    320 | 300 | 350

warning: group (run_id: 2026-10-09T14:00:00Z-sweep-001, metric: tokens_per_sec, config_hash: fc046d91d2e2) has fewer than 3 ok seeds (n_ok = 2)
```

`--format json` emits a machine-readable result for the dashboard; `text`
remains the default. Rejections still go to stderr in both formats and the
exit codes are unchanged. JSON retains full hashes and the original stable
group-key order, including with `--group-by`. The JSON shape:

- `output_version` — a top-level integer (currently `2`) that changes whenever
  the shape of this output changes. Version 2 adds the full `config` object to
  every group; version 1 only included `config_hash` and optional `group_by` values.
- `accepted`, `rejected`, `superseded` — the same counts as the text header.
- `complete` — `false` whenever any record was rejected. It means "no input was
  rejected", not "no known limitations". Accepted unsuccessful measurements,
  excluded aggregate records, and fewer-than-3-seeds warnings do not make it false.
- `rejections` — one object per rejected record, sorted by line: `line`,
  `field` (`<json>` for unparseable lines), and `reason`.
- `groups` — the aggregate groups from accepted, effective records, including
  partial results when `complete` is `false`. Groups are empty only when there
  are no effective records eligible for aggregation. Each group carries the key
  fields (`run_id`, `metric`, `unit`,
  `config_hash`, `dataset_fingerprint`, `evaluator_fingerprint`), the full `config`
  object as written in the record (key order may change, but values are unchanged,
  including integers versus floats such as `8` versus `8.0`), `n_attempted`,
  `n_ok`, `median` / `min` / `max` (`null` when `n_ok = 0`), and the
  fewer-than-3-seeds warning as `warning`. `config` is present even without
  `--group-by`, so the dashboard can label each group directly. With `--group-by`,
  each group also carries a `group_by` object with the chosen config keys: an explicit `null`
  serializes as `null`, an absent key is omitted from the object.

```sh
cargo run -- --input data/v2/identity.jsonl --format json
```

```json
{
  "output_version": 2,
  "accepted": 3,
  "rejected": 3,
  "superseded": 1,
  "complete": false,
  "rejections": [
    {
      "line": 2,
      "field": "measurement_key",
      "reason": "duplicate measurement (first seen on line 1)"
    },
    {
      "line": 4,
      "field": "supersedes_record_id",
      "reason": "dangling reference: record 'missing-record' was not accepted earlier"
    },
    {
      "line": 6,
      "field": "supersedes_record_id",
      "reason": "target record 'identity-original' has a different measurement_key"
    }
  ],
  "groups": [
    {
      "run_id": "00000000-0000-4000-8000-000000000010",
      "metric": "accuracy",
      "unit": "ratio",
      "config_hash": "fc046d91d2e2a564a21b50cbdc73ff4cfe50221c35af239573f263c5b469a5cf",
      "config": {
        "batch_size": 8
      },
      "dataset_fingerprint": "null",
      "evaluator_fingerprint": "null",
      "n_attempted": 2,
      "n_ok": 2,
      "median": 0.8,
      "min": 0.7,
      "max": 0.9,
      "warning": true
    }
  ]
}
```

A group from `--input data/v2/sweep.jsonl --format json --group-by batch_size`
looks like this:

```json
{
  "run_id": "2026-10-09T14:00:00Z-sweep-001",
  "metric": "tokens_per_sec",
  "unit": "tokens_per_sec",
  "config_hash": "297b1e2373ca8181a16df4aaa7b57e6f27a232e43aa658d798d97e9246dbe749",
  "config": {
    "batch_size": 32
  },
  "dataset_fingerprint": "null",
  "evaluator_fingerprint": "null",
  "group_by": {
    "batch_size": 32
  },
  "n_attempted": 3,
  "n_ok": 3,
  "median": 320.0,
  "min": 300.0,
  "max": 350.0,
  "warning": false
}
```

The fixture in `data/v2/valid.jsonl` covers absent, populated, and null optional
blocks. The fixture in `data/v2/invalid.jsonl` covers one defect per line
(missing field, wrong type, bad enum, malformed block, bad JSON). The identity fixture
covers an accidental duplicate, a valid correction, a dangling correction, and a correction
that targets a different measurement. The `data/v2/record-id-reuse.jsonl` fixture
covers a reused record ID with a different seed followed by a valid correction
of the original. The sweep fixture in `data/v2/sweep.jsonl` covers
a batch size sweep across seeds with an OOM measurement, an aggregate record, and hand-computed
medians. Expected outcomes are documented in `data/v2/README`.
The `sweep-partial.jsonl` fixture adds one malformed line to the sweep: the
same three groups and medians are shown, with a rejection notice in text mode
and `complete: false` in JSON mode, and exit code 1.

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
