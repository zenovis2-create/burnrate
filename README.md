# burnrate

[![CI](https://github.com/zenovis2-create/burnrate/actions/workflows/ci.yml/badge.svg)](https://github.com/zenovis2-create/burnrate/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/zenovis2-create/burnrate)](https://github.com/zenovis2-create/burnrate/releases/latest)

**`htop` for coding-agent spend.** See what Claude Code and Codex cost, which
sessions burn the most tokens, and where agents keep reading the same files.

![burnrate terminal demo](assets/demo.gif)

Local-only: burnrate reads the logs already on your machine. Nothing is uploaded.

If burnrate helps you catch an expensive agent loop, please **star the repo** so
other builders can find it.

## Install

```sh
cargo install --git https://github.com/zenovis2-create/burnrate
```

Prebuilt macOS, Linux, and Windows binaries are available on the
[latest release](https://github.com/zenovis2-create/burnrate/releases/latest).

## Use

```sh
burnrate report     # sessions ranked by estimated cost
burnrate report --json > burnrate.json # machine-readable report
burnrate tui        # interactive terminal dashboard
burnrate --days 30  # widen the window
burnrate --demo tui # try it with synthetic, privacy-safe data
```

burnrate automatically reads:

- Claude Code: `~/.claude/projects/**/*.jsonl`
- Codex: `~/.codex/sessions/**/*.jsonl`

The `--days` window follows each session log's latest filesystem activity, so a
long-running session is not dropped just because it started before the window.

In the TUI, use the arrow keys or `j`/`k` to move, Page Up/Page Down to jump,
Home/End to reach the edges, and `q` or Escape to quit.

### JSON reports

`burnrate report --json` returns the generation time, reporting window,
aggregate totals, per-source totals, and cost-ranked session details. This is
intended for shell scripts, scheduled snapshots, and custom dashboards.

## What it shows

- estimated API-rate cost by session and harness
- input, cached-input, and output token totals
- cache-write token totals when reported by the harness
- cache share (reported separately from waste)
- redundant Claude Code `Read` tool calls and the worst repeated file
- the most expensive sessions first

## Support

| Harness | Spend | Cache | Repeated file reads |
| --- | --- | --- | --- |
| Claude Code | yes | yes | yes |
| Codex | yes | yes | not yet |
| Cursor | planned | planned | planned |
| Gemini CLI | planned | planned | planned |

## Accuracy and privacy

Costs are estimates from a small hardcoded price table, not invoice data. Log
formats and provider prices change; verify important numbers against your bill.
Recognized models use model-specific standard API rates; unrecognized usage is
shown as unpriced and contributes $0 rather than borrowing another model's
rate. Claude sessions that switch models are priced per model and display the
model responsible for the most tokens. Codex rollout files may repeat
cumulative totals across forked threads.

Parsing happens locally and burnrate has no telemetry or network client.

## What this is not

- not a memory server
- not an agent development environment
- not an MCP kitchen sink
- not a cloud billing service

## Roadmap

- Cursor and Gemini CLI parsers
- editable price table
- package-manager installs

## Development

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo audit --deny warnings
```

Pull requests run these checks and release builds on Linux, macOS, and Windows.

MIT licensed.
