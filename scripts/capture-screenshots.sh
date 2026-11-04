#!/usr/bin/env bash
# Capture one screenshot per page for the README, under realistic load so the
# graphs show something worth looking at.
#
# The window is made fullscreen (the same thing Super+F does) before capturing,
# so no desktop panel, status bar or other window appears in the image.
#
# Requires Hyprland (hyprctl), grim and jq. Run from the repository root.
set -euo pipefail

OUT="${1:-docs/screenshots}"
SHOT_DELAY="${SHOT_DELAY:-26}"
SHOT_WORKSPACE="${SHOT_WORKSPACE:-9}"
# Must be on a real drive, not a RAM-backed /tmp, or there is no disk activity.
LOAD_FILE="${LOAD_FILE:-$PWD/target/perfview-load.bin}"
mkdir -p "$OUT"

cargo build --release

cleanup() {
  pkill -f 'target/release/perfview' 2>/dev/null || true
  pkill -f 'perfview-load' 2>/dev/null || true
  pkill -x glxgears 2>/dev/null || true
  rm -f "$LOAD_FILE"
}
trap cleanup EXIT

start_load() {
  # Processor: a handful of busy threads, not the whole machine.
  for _ in $(seq 1 5); do
    (exec -a perfview-load bash -c 'while :; do :; done') &
  done
  # Storage: write then re-read a large file.
  (exec -a perfview-load bash -c "
    while :; do
      dd if=/dev/urandom of='$LOAD_FILE' bs=1M count=1200 oflag=direct 2>/dev/null
      dd if='$LOAD_FILE' of=/dev/null bs=1M iflag=direct 2>/dev/null
    done") &
  # Graphics: an uncapped spinning cube is enough to move the needle.
  vblank_mode=0 glxgears >/dev/null 2>&1 &
}

capture() {
  local page="$1" name="$2" expected="$3"
  echo "Capturing $name…"
  ./target/release/perfview --fresh --page "$page" --refresh 250 >/dev/null 2>&1 &

  # Wait for the window to actually exist rather than guessing with a sleep.
  # perfview names the current page in its title, which is what we match on.
  local waited=0
  until [ "$(window_title)" = "perfview — $expected" ]; do
    sleep 0.5
    waited=$((waited + 1))
    if [ "$waited" -gt 40 ]; then
      echo "perfview did not open the $expected page" >&2
      exit 1
    fi
  done

  # Put the window on an empty workspace of its own, then make it fullscreen.
  # `fullscreenstate 2` sets fullscreen explicitly rather than toggling it the
  # way the `fullscreen` dispatcher (Super+F) does, so repeated captures cannot
  # end up un-fullscreening the window and letting the status bar back in.
  hyprctl dispatch movetoworkspacesilent \
    "$SHOT_WORKSPACE,class:dev.crxigrxberts.perfview" >/dev/null
  hyprctl dispatch workspace "$SHOT_WORKSPACE" >/dev/null
  hyprctl dispatch focuswindow class:dev.crxigrxberts.perfview >/dev/null
  sleep 0.5
  hyprctl dispatch fullscreenstate 2 -1 >/dev/null
  # Park the pointer off-screen so the cursor is not composited into the shot.
  hyprctl dispatch movecursor 20000 20000 >/dev/null
  sleep "$SHOT_DELAY"

  # Capture the monitor the window is filling, which now *is* the window.
  local monitor geometry
  monitor=$(hyprctl clients -j |
    jq -r '.[] | select(.class|test("perfview";"i")) | .monitor' | head -1)
  geometry=$(hyprctl monitors -j |
    jq -r --argjson m "$monitor" \
      '.[] | select(.id == $m) | "\(.x),\(.y) \(.width)x\(.height)"')

  sleep 0.5
  grim -g "$geometry" "$OUT/$name.png"

  pkill -f 'target/release/perfview' || true
  # Wait for it to actually go, so the next capture cannot match this window.
  while [ -n "$(window_title)" ]; do sleep 0.2; done
}

window_title() {
  hyprctl clients -j |
    jq -r '.[] | select(.class|test("perfview";"i")) | .title' | head -1
}

start_load
capture overview 01-overview Overview
capture gpu 02-graphics Graphics
capture cpu 03-processor Processor
capture memory 04-memory Memory
capture storage 05-storage Storage

echo "Screenshots written to $OUT"
