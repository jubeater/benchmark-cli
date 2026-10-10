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

The fixture in `data/v2/valid.jsonl` covers absent, populated, and null optional
blocks. The fixture in `data/v2/invalid.jsonl` covers one defect per line
(missing field, wrong type, bad enum, malformed block, bad JSON). Expected
outcomes are documented in `data/v2/README`. The identity fixture covers an
accidental duplicate, a valid correction, a dangling correction, and a correction
that targets a different measurement.

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
