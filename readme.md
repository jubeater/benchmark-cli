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
Accepted records: 3
Rejected records: 0
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

Records with a non-null `supersedes_record_id` pass through this duplicate check.
Correction target validation and replacement are deferred to ticket #5.

The fixture in `data/v2/valid.jsonl` covers absent, populated, and null optional
blocks. The fixture in `data/v2/invalid.jsonl` covers one defect per line
(missing field, wrong type, bad enum, malformed block, bad JSON). Expected
outcomes are documented in `data/v2/README`. The identity fixture covers an
accidental duplicate and correction pass-through.

Run the checks with:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```
