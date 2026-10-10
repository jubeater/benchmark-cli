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
with code 2. The additional pinned schema rules are planned in ticket #3.

The fixture in `data/v2/valid.jsonl` covers absent, populated, and null optional
blocks. The fixture in `data/v2/invalid.jsonl` covers one defect per line
(missing field, wrong type, bad enum, malformed block, bad JSON). Expected
outcomes are documented in `data/v2/README`.

Run the checks with:

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```
