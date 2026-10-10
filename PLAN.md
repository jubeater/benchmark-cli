# PLAN — eval-schema v2 validator & aggregator

**Goal:** turn `benchmark-cli` into the schema validator and seed aggregator for
every project's `results.jsonl`, per eval-schema v2.

**Assumptions**
- Single writer (the owner), records follow the agreed schema. Happy path first;
  defensive corner cases are listed under *Known limitations*, not handled.
- v2 only. No v1 support, no existing data to migrate — schema precedes results.

**In scope:** v2 validation, record identity (duplicate rejection + corrections),
aggregation across seeds, text + JSON output.

**Out of scope (later, layered on the aggregator):** `--baseline` diffing,
bootstrap CIs, cost/latency rollups.

**Estimate:** ~10 h total. W1 = Stages 0–3 (~6 h); W2 = Stages 4–5.

**Gate for every stage:** `cargo test` green,
`cargo clippy --all-targets -- -D warnings` clean, committed.

---

## Starting point (HEAD `4a3e20e`)

- `analysis.rs` is a stub (only the parse-error count survives); the old
  per-request analysis was removed and doesn't apply to per-metric v2 records.
- `validator.rs` checks core presence/types, enums and block structure, with
  9 tests — but is not wired into `main` (dead code). Errors carry no field/reason.
- `model.rs` lacks `supersedes_record_id` and `unit`.
- 15 tests total (9 validator, 4 input, 2 report). Legacy log/pricing data in `data/`.

---

## Stage 0 — Pin the hash rules in eval-schema (0.5 h) — DONE 2026-10-09

**Replace** (don't append to) the canonical-serialization section of eval-schema:

- `measurement_key = sha256(canonical JSON array
  [run_id, metric, config_hash, seed, dataset_fp, evaluator_fp])`.
- `dataset_fp` / `evaluator_fp` = sha256 of the canonical JSON of the listed
  fields (`name, version, preprocessing, split, sample_ids_hash` /
  `kind, name, version, prompt_version`). **Absent fields are omitted**, not
  written as null. Absent block — or block set to `null` — ⇒ the string `"null"`.
- Canonical JSON = `serde_json` serialization with recursively sorted keys.
  Integer vs float is preserved (`8` ≠ `8.0`). Reject `-0.0`.
- Remove the old "integers bare" rule and the Python one-liner / JCS guidance.
- Add required core field `unit`, closed vocabulary:
  `ms, s, tokens_per_sec, items_per_sec, mb, gb, ratio, count, usd, score`.
  Time/throughput/memory units (`ms, s, tokens_per_sec, items_per_sec, mb, gb`)
  ⇒ `env` block required.
- Delete § Migration from v1.

**Exit**
- [x] Schema doc contains the rules above and none of the replaced wording.
- [x] Golden hashes in the doc (config, evaluator, measurement_key), one of them an `exact_match` evaluator
      **without** `prompt_version`. Stage 3 tests check against them.

## Stage 1 — Cleanup & fixtures (1 h)

- Delete legacy `data/sample.jsonl`, `data/pricing.json`, `data/test/*` and any
  remaining pricing/request-log code.
- Add `data/v2/`:
  - `valid.jsonl` — conformant records, including blocks.
  - `invalid.jsonl` — a few lines, one defect each.
  - `sweep.jsonl` — batch 1/8/32 × 3 seeds, one `oom`, one aggregate record.
  - `identity.jsonl` — accidental duplicate, valid correction, dangling correction.
  - `data/v2/README` — expected outcome per fixture.
- Point the `input.rs` file test at a v2 fixture.

**Exit**
- [ ] `grep -ri pricing src data readme.md` is empty.
- [ ] Every fixture's expected outcome is written in `data/v2/README`.

## Stage 2 — Wire validation (1.5 h)

- `main`: parse → validate. Rejections to stderr as `line N: <field>: <reason>`.
- Add `supersedes_record_id` and `unit` to the model; enforce `unit → env`.
- Exit code 1 if any record was rejected, 2 on I/O error.

**Exit**
- [ ] `valid.jsonl` → 0 rejections, exit 0.
- [ ] Each line of `invalid.jsonl` reported with its expected field; exit 1.

## Stage 3 — Record identity (2–3 h)

- `canonical.rs`: canonical bytes, sha256, `config_hash`, fingerprints,
  `measurement_key`. **Build fingerprints from the original JSON `Value`, not the
  typed structs** — `Option` collapses absent and null.
- `identity.rs`: single pass in file order applying the schema's six-row
  correction table. Chains resolve to the last record.
- **Exactly one effective record per `measurement_key`** goes to aggregation.

**Exit**
- [ ] Golden hashes from Stage 0 match.
- [ ] Reordering object keys doesn't change any hash.
- [ ] Three seeds, same metric/config → three distinct keys.
- [ ] `identity.jsonl`: duplicate and dangling correction rejected; the valid
      correction replaces its original in the effective set.

**— End of W1 —**

## Stage 4 — Aggregation (2 h)

- Group = `(run_id, metric, unit, config_hash, dataset_fp, evaluator_fp)`.
  Full config is in the key, so a sweep splits per config on its own.
- `--group-by <keys>` selects which config keys are shown as columns
  (presentation only; deferrable).
- Per group: `n_attempted`, `n_ok`, `median` / `min` / `max` over `ok` values.
  - Even count: median = mean of the two middle values.
  - `n_ok = 0` → null stats.
- Records with `config.aggregate` are excluded.
- Warn when `n_ok < 3` (three-seed rule).

**Exit**
- [ ] `sweep.jsonl` → 3 groups with hand-computed medians.
- [ ] The `oom` record is counted in `n_attempted`, not in the median
      (also covers the even-count median).
- [ ] The aggregate record is excluded.
- [ ] Unit test: all-failed group → `n_ok = 0`, null stats.

## Stage 5 — Output & end-to-end tests (1.5 h)

- Text table by default; `--format json` for the dashboard.
- Remove the old report types; rewrite `report.rs` and the readme.
- `tests/cli.rs` (`assert_cmd`) runs the binary on every fixture.

**Exit**
- [ ] Every fixture has an E2E test asserting its expected outcome.
- [ ] JSON output parses with `jq`.

---

## Known limitations (copy into readme)

Not handled, by design — none occur if records follow the schema:

- Duplicate keys inside one JSON line: last value wins (`serde_json` behavior).
- Integers beyond 64-bit: rounded to `f64`; distinct values can hash the same.
- Two corrections of the same record (fork): the later one in the file wins.
- Unknown keys inside blocks are accepted (top-level is validated).
- Model/env consistency within a group is not checked.
- LLM-run rules (`model.prompt_version`, `cost` required) and judge-calibration
  rules are not enforced — there is no run-kind field to key them on.

## Later

- `--baseline <run_id>`: pair groups, check fingerprint compatibility and
  run-wide `git_dirty`, report descriptive deltas.
- Bootstrap CIs — only meaningful with more than 3 seeds.
- Cost/latency rollups — needs a decision on whether cost belongs to a
  measurement or an execution (one run repeats its cost block per metric).

## Dependencies

`sha2`; dev: `assert_cmd`. (`uuid`/`chrono` only if stricter field checks are added.)
