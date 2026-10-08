#!/usr/bin/env bash
# Prints the ptc pomodoro for the tmux status line (prints nothing when there is nothing to show).
file="${PTC_STATUS_FILE:-${XDG_RUNTIME_DIR:-/tmp}/ptc-status.json}"
[ -r "$file" ] || exit 0

# Value of a top-level key of the status file (works for the pretty-printed and the compact form).
get() { grep -o "\"$1\": *\"\{0,1\}[^\",}]*" "$file" | head -n1 | sed 's/^[^:]*: *"\{0,1\}//'; }

state=$(get state)
case "$state" in
  work) label="Focus" ;;
  short_break) label="Break" ;;
  long_break) label="Long break" ;;
  *) exit 0 ;; # idle, off or unreadable
esac

paused=$(get paused)
waiting=$(get waiting)
ends_at=$(get ends_at)
now=$(date +%s)

if [ -n "$ends_at" ] && [ "$paused" != true ] && [ "$waiting" != true ]; then
  # A running phase far past its end means ptc died without saying goodbye.
  [ "$now" -gt $((ends_at + 5)) ] && exit 0
  left=$((ends_at - now))
  [ "$left" -lt 0 ] && left=0
else
  left=$(get remaining_seconds)
  left=${left:-0}
fi

suffix=""
[ "$paused" = true ] && suffix=" (paused)"
[ "$waiting" = true ] && suffix=" (ready)"
printf '🍅 %s %02d:%02d%s\n' "$label" $((left / 60)) $((left % 60)) "$suffix"
