# burnrate

[![CI](https://github.com/zenovis2-create/burnrate/actions/workflows/ci.yml/badge.svg)](https://github.com/zenovis2-create/burnrate/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/zenovis2-create/burnrate)](https://github.com/zenovis2-create/burnrate/releases/latest)

**`htop` for coding-agent spend.** One local Rust binary that reads your
Claude Code and Codex logs and shows estimated API-rate costs, prompt cache
usage, and repeated file reads. Unknown pricing and incomplete input stay
visible; these are local estimates, not your bill.

![burnrate terminal demo](assets/demo.gif)

_Recorded from the real TUI with the built-in synthetic dataset; no user logs
are shown._

| | |
|---|---|
| **Install** | `cargo install --git https://github.com/zenovis2-create/burnrate` (or [prebuilt binaries](https://github.com/zenovis2-create/burnrate/releases/latest)) |
| **Privacy** | local-only — no telemetry, no network calls, logs never leave your machine |
| **License** | MIT, zero runtime dependencies |

## Install

```sh
cargo install --git https://github.com/zenovis2-create/burnrate
```

Prebuilt macOS, Linux, and Windows binaries are on the
[latest release](https://github.com/zenovis2-create/burnrate/releases/latest).

No Rust toolchain? The demo mode runs on synthetic, privacy-safe data:

```sh
burnrate --demo report
```

## Use

```sh
burnrate report            # sessions ranked by estimated cost
burnrate report --json     # machine-readable report
burnrate tui               # interactive terminal dashboard
burnrate --days 30         # widen the window
burnrate --demo tui        # try it with synthetic, privacy-safe data
```

burnrate automatically reads:

- Claude Code: `~/.claude/projects/**/*.jsonl`
- Codex: `~/.codex/sessions/**/*.jsonl`

The `--days` window follows each session log's latest filesystem activity, so a
long-running session is not dropped just because it started before the window.
Totals include that session's entire recorded usage, **not just events inside
the window**. The TUI is a snapshot, not a live watcher; its timestamp shows when
the snapshot was collected. Relaunch it to rescan.

In the TUI, use the arrow keys or `j`/`k` to move, Page Up/Page Down to jump,
Home/End to reach the edges, and `q` or Escape to quit.

### JSON reports

`burnrate report --json` returns the generation time, reporting window,
aggregate totals, per-source totals, and cost-ranked session details. This is
intended for shell scripts, scheduled snapshots, and custom dashboards.

`cost_basis` identifies a standard-API-rate known subtotal. The existing
`total_cost_usd` and per-session `cost_usd` fields retain their numeric shape;
unknown usage is excluded from those subtotals, not priced as free.
`unpriced_sessions`, `unpriced_tokens`, and `unpriced_models` disclose that gap.
`billing_status` and `quota_status` are `not_observed`: local logs are not an
invoice or a provider quota API. `window_basis` describes the activity-based
selection above. `data_source` distinguishes local logs from the synthetic demo.

The `scan` object reports roots found, recently touched JSONL files discovered,
files opened, sessions parsed, opened files without recognized usage, I/O errors,
and malformed JSON lines. Demo reports use `scan: null`, not fake scan counts.
An opened file can still have read errors. Tables and the TUI mark scans with
I/O or JSON errors as `partial input`. No detected errors is **not** a claim
of complete coverage: valid but unsupported event shapes, deleted history,
other devices, and fork duplication are not certified by these counters.

## What it shows

- estimated API-rate cost per session and per harness
- input, cached-input, and output token totals
- cache-write token totals when reported by the harness
- cache share (reported separately from waste)
- repeated Claude Code `Read` tool calls and the most re-read file (a signal,
  not proof of waste; ranges and intervening edits are not compared)
  (a separate manual analysis reported **97% of file-read volume as re-reads
  of the same file** in one real Codex session; automatic Codex re-read
  detection is not yet supported, and this is not a general waste rate)
- sessions ranked by known estimated subtotal

## How it compares

[ccusage](https://github.com/ccusage/ccusage) is a broad multi-harness reporter
with daily, weekly, monthly, and session views.
[CodeBurn](https://github.com/getagentseal/codeburn) covers more tools and adds
terminal, desktop, web, and optimization surfaces. burnrate is deliberately
narrower: one native CLI binary and one fast path from session cost to
within-session Claude Code re-read waste.

| | burnrate | ccusage | CodeBurn |
|---|---|---|---|
| Per-session cost reporting | yes | yes | yes |
| Cache metrics | share + tokens per session | cache read/write tokens | cache hit per row |
| Dedicated re-read waste analysis | within-session (Claude Code) | not documented | yes (`optimize`) |
| Interactive TUI | yes | no | yes |
| Default CLI runtime | native Rust binary | `npx` / other package runners | Node.js 22.13+ |
| Sources documented (August 2026) | 2 | 15 | 41 |

Choose the broader tools when source coverage or extra surfaces matter more;
choose burnrate when a small native binary and focused drill-down are the goal.

## Support matrix

| Harness | Spend | Cache | Repeated file reads |
| --- | --- | --- | --- |
| Claude Code | yes | yes | yes |
| Codex | yes | yes | not yet |
| Cursor | planned | planned | planned |
| Gemini CLI | planned | planned | planned |

## Accuracy and privacy

Costs are estimates from a small hardcoded price table, not invoice data. Log
formats and provider prices change; verify important numbers against your bill.
Recognized models use model-specific standard API rates. Unrecognized usage
remains unpriced instead of borrowing another model's rate. Table and TUI rows
use `+?` to mark unknown cost alongside the known subtotal; `$0.00+?` does not
mean free. Malformed records are counted and skipped, never used to infer a
model price. Claude sessions that switch models are priced per model and display the
model responsible for the most tokens. Codex rollout files may repeat
cumulative totals across forked threads.

Parsing happens locally; burnrate has no telemetry and no network client.

## What this is not

- not a memory server
- not an agent development environment
- not an MCP kitchen sink
- not a cloud billing service

## Roadmap

- **Cursor and Gemini CLI parsers** (highest demand — PRs welcome)
- Redundant re-read detection for Codex
- Editable price table (JSON)
- Package-manager installs (brew, cargo-binstall)

If burnrate catches an expensive loop in your logs, a star helps other
builders find it.

## Contributing

Issues: use the [bug report](.github/ISSUE_TEMPLATE/bug_report.yml) or
[feature request](.github/ISSUE_TEMPLATE/feature_request.yml) templates.
For parser bugs, attach a **redacted** snippet of the log line that breaks —
one session's worth is plenty, no need to paste whole logs.

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo audit --deny warnings
```

Pull requests run these checks and release builds on Linux, macOS, and Windows.

MIT licensed.
