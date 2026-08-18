# burnrate

**htop for coding-agent spend.**

Your coding agent just told you "done." burnrate tells you what it cost.

Point it at your Claude Code / Codex logs and see, in a live TUI:

- dollars burned — this session, today, this week
- tokens wasted on re-reads and cache churn
- which model each session actually ran
- a one-line verdict: *this session cost $4.12 and 61% of it was the agent re-reading `package-lock.json`*

Local-only. Your logs never leave the machine.

## Install

```sh
# coming soon — GitHub Releases + brew tap
cargo install --git https://github.com/zenovis2-create/burnrate
```

## Usage

```sh
burnrate            # TUI — sessions ranked by cost
burnrate report     # plain-text table (default)
burnrate --days 30  # widen the window
```

## What this is not

- not a memory server
- not an ADE (agent development environment)
- not an MCP kitchen sink
- not a cloud service

It is a thin, fast, local cost lens on logs you already have.

## Status

v0.1 — Claude Code + Codex parsers, price table (approximate, hardcoded), TUI.
Cursor / Gemini CLI parsers, waste detector, `report --json`, and `prices pull` are next.

**v0 caveat:** Codex rollout files re-report cumulative thread totals, so fork chains
count the shared history once per fork. Numbers are API-rate estimates, not bills.

## License

MIT
