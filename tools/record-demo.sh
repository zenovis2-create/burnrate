#!/bin/sh
set -eu

demo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$demo_root"

for demo_tool in cargo asciinema agg expect; do
    if ! command -v "$demo_tool" >/dev/null 2>&1; then
        printf 'missing required tool: %s\n' "$demo_tool" >&2
        exit 1
    fi
done

demo_cast=$(mktemp "${TMPDIR:-/tmp}/burnrate-demo.XXXXXX")
trap 'rm -f -- "$demo_cast"' EXIT HUP INT TERM

cargo build --release

env -u NO_COLOR TERM=xterm-256color COLORTERM=truecolor \
    asciinema record \
    --headless \
    --overwrite \
    --return \
    --window-size 110x24 \
    --title 'burnrate --demo tui' \
    --command "expect -c 'set timeout 10; log_user 0; spawn ./target/release/burnrate --demo tui; log_user 1; after 1200 {send -- j}; after 2200 {send -- j}; after 3200 {send -- k}; after 4500 {send -- q}; expect eof'" \
    "$demo_cast"

agg \
    --theme github-dark \
    --bold-is-bright \
    --font-size 15 \
    --line-height 1.25 \
    --fps-cap 12 \
    --idle-time-limit 1 \
    --last-frame-duration 2 \
    --select 0..4.4 \
    "$demo_cast" \
    assets/demo.gif

printf 'recorded assets/demo.gif\n'
