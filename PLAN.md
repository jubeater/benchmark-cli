# PLAN — eval-schema v2 validator & aggregator

**Status: MVP complete (2026-10-10).** All stages shipped (issues #1–#8), the
review follow-ups are merged (#10–#14, PR #15), and 77 tests pass with
`cargo clippy --all-targets -- -D warnings` clean.

**Goal:** turn `benchmark-cli` into the schema validator and seed aggregator for
every project's `results.jsonl`, per eval-schema v2.

**Assumptions**
- Single writer (the owner), records follow the agreed schema. Happy path first;
  defensive corner cases are listed under *Known limitations*, not handled.
- v2 only. No v1 support, no existing data to migrate — schema precedes results.

**In scope (done):** v2 validation, record identity (duplicate rejection +
corrections), aggregation across seeds, text + JSON output.

**Out of scope (next):** `--baseline` diffing, bootstrap CIs, cost/latency
rollups — see *Next*.

**Behavior reference:** `readme.md` documents current behavior and the JSON
output shape; `data/v2/README` documents each fixture's expected outcome. This
file records what was planned, what shipped, and what's left.

---

## Starting point (HEAD `4a3e20e`, before the work)

- `analysis.rs` was a stub; the old per-request log analysis didn't apply to
  per-metric v2 records.
- `validator.rs` checked core presence/types, enums and block structure, but was
  not wired into `main`. Errors carried no field or reason.
- `model.rs` lacked `supersedes_record_id` and `unit`.
- 15 tests. Legacy log/pricing data in `data/`.

---

## Stages — all done

| Stage | What | Issues | Commits |
|---|---|---|---|
| 0 | Pin the hash rules in eval-schema; add `unit`; drop v1 | — (schema doc) | — |
| 1 | Remove legacy pipeline; v2 fixtures; E2E harness | #1 | `804cc21` |
| 2 | Wire validation; `line N: field: reason`; exit codes; new schema rules | #2, #3 | `0d2e0a9`, `5b8a046` |
| 3 | Canonical hashing, `measurement_key`, duplicates, corrections | #4, #5 | `eb251de`, `d59a07a` |
| 4 | Seed statistics per group; `--group-by` columns | #6, #7 | `e05b875`, `079d8c8` |
| 5 | `--format json` for the dashboard | #8 | `6a1fce2` |

### Stage 0 — Pin the hash rules in eval-schema
- [x] `measurement_key = sha256(canonical JSON array [run_id, metric, config_hash, seed, dataset_fp, evaluator_fp])`.
- [x] Fingerprints take only their listed fields; absent fields omitted; absent or
      `null` block ⇒ `"null"`. Canonical JSON = `serde_json`, sorted keys,
      `8` ≠ `8.0`, `-0.0` rejected. Old "integers bare" / Python / JCS wording removed.
- [x] Required `unit` with a closed vocabulary; time/throughput/memory units require `env`.
- [x] § Migration from v1 replaced by "No v1 support".
- [x] Three reference hashes in the doc (config, evaluator without `prompt_version`,
      `measurement_key`).

### Stage 1 — Cleanup & fixtures
- [x] No pricing or request-log code or data remains.
- [x] `data/v2/` fixtures with expected outcomes in `data/v2/README`.

### Stage 2 — Validation wired in
- [x] Valid fixture → 0 rejections, exit 0.
- [x] Each invalid line reported with its expected field; exit 1; I/O error exit 2.
- [x] `unit`, `unit → env`, `supersedes_record_id`, finite `value` when `ok`,
      unknown top-level keys and `-0.0` in `config` rejected.

### Stage 3 — Record identity
- [x] The schema's reference hashes are reproduced (bytes and hash).
- [x] Key order never changes a hash; `null` ≠ absent; seeds, datasets and
      evaluators all give distinct keys.
- [x] Every row of the correction table is tested; chains resolve to the last
      record; exactly one effective record per `measurement_key` reaches aggregation.

### Stage 4 — Aggregation
- [x] Group = `(run_id, metric, unit, config_hash, dataset_fp, evaluator_fp)`.
- [x] Sweep fixture gives the hand-computed medians (102 / 205 / 320); the `oom`
      seed counts as attempted but not in the median; the aggregate record is excluded.
- [x] All-failed group → `n_ok = 0`, null statistics; warning when `n_ok < 3`.
- [x] `--group-by` shows chosen config keys as columns (presentation only).

### Stage 5 — Output & E2E
- [x] Text table by default; `--format json` with `output_version`.
- [x] Every fixture has an E2E test; JSON snapshot tests compare the full output.

---

## Review follow-ups — all done (PR #15)

Found in the review of #1–#8 and fixed one ticket at a time.

| Issue | Type | Origin | Fix | Commit |
|---|---|---|---|---|
| #10 | Bug | #5 | A reused `record_id` is rejected; it used to overwrite the correction lookup, so a valid correction was rejected | `9d783b7` |
| #11 | Deviation | #6 vs #8 | Accepted records are aggregated even when some are rejected; `complete: false` marks a shown-but-partial result; exit code stays 1 | `0761424` |
| #12 | Gap | #2 | Empty and whitespace-only lines are skipped; line numbers stay physical | `c8aa56d` |
| #13 | Improvement | #6, #7 | Text view shows 12-character hashes; rows sort by `--group-by` values | `3dfeb16` |
| #14 | Gap | #8 | Every JSON group carries its full `config`; `output_version` is 2 | `97141f1` |

## Changes from the original plan

- **Partial results instead of all-or-nothing.** The plan didn't say what happens
  when some records are rejected; the first implementation showed no statistics at
  all. Now accepted records are always aggregated, and the result is marked
  incomplete (text notice, JSON `complete: false`, exit 1).
- **`record_id` must be unique.** Not in the original plan; added after the review
  showed that reuse breaks correction lookups.
- **Blank lines are ignored** rather than rejected.
- **JSON output is version 2**: each group includes `config`.
- **No `assert_cmd` / `insta`.** E2E tests run the built binary with
  `std::process::Command` and compare full output strings. Only `sha2` was added
  as a dependency.

---

## Known limitations

Not handled, by design — none occur if records follow the schema:

- Duplicate keys inside one JSON line: last value wins (`serde_json` behavior).
- Integers beyond 64-bit: rounded to `f64`; distinct values can hash the same.
- Two corrections of the same record (fork): the later one in the file wins.
- Unknown keys inside blocks are accepted (top-level is validated).
- `record_id` and `timestamp` formats (UUID, RFC 3339) are not checked.
- Model/env consistency within a group is not checked.
- LLM-run rules (`model.prompt_version`, `cost` required) and judge-calibration
  rules are not enforced — there is no run-kind field to key them on.

## Next

Not started. Each builds on the aggregator's groups.

1. **`--baseline <run_id>`** — pair candidate and baseline groups within a project
   on metric, unit and `--group-by` values (exactly one counterpart; ambiguity in
   either direction is an error). Report `not_comparable` when dataset or evaluator
   fingerprints differ, `disqualified` when either *run* has any `git_dirty` record,
   `exploratory` when `declared_before_run` is false; otherwise descriptive deltas
   (percentage `n/a` when the baseline median ≤ 0). No pass/fail until there are
   enough seeds for it to mean something.
2. **Bootstrap CIs** — only meaningful with more than 3 seeds: at n = 3 a "95%"
   percentile interval is the sample range and covers the true median only ~75% of
   the time.
3. **Cost/latency rollups** — **needs a decision first:** does cost belong to a
   measurement or to an execution? One run repeats its `cost` block on every metric
   record, so summing per record counts the same spend several times.

## Dependencies

`sha2`. (`uuid`/`chrono` only if stricter field checks are added.)
