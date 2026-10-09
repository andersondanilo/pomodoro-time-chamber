#!/usr/bin/env bash
# Prints the ptc pomodoro and the current task for the tmux status line (prints nothing when there
# is nothing to show). PTC_TMUX_MAX_TASK (default 30) is the longest task name shown.
# Count and cut the task name by characters, not bytes, whatever locale tmux runs this in.
export LC_ALL=C.UTF-8
file="${PTC_STATUS_FILE:-${XDG_RUNTIME_DIR:-/tmp}/ptc-status.json}"
[ -r "$file" ] || exit 0

# Value of a top-level key of the status file (works for the pretty-printed and the compact form).
get() { grep -o "\"$1\": *\"\{0,1\}[^\",}]*" "$file" | head -n1 | sed 's/^[^:]*: *"\{0,1\}//'; }

state=$(get state)
now=$(date +%s)

# Idle: nothing to show, except the idle time after a break that finished by itself
# (the app's "Idle for MM:SS"). A plain idle has no `idle_since` and stays hidden.
if [ "$state" = idle ]; then
  idle_since=$(get idle_since)
  [ -z "$idle_since" ] && exit 0
  idle=$((now - idle_since))
  [ "$idle" -lt 0 ] && idle=0
  if [ "$idle" -ge 3600 ]; then
    printf '🍅 Idle for %d:%02d:%02d\n' $((idle / 3600)) $((idle % 3600 / 60)) $((idle % 60))
  else
    printf '🍅 Idle for %02d:%02d\n' $((idle / 60)) $((idle % 60))
  fi
  exit 0
fi

case "$state" in
  work) label="Focus" ;;
  short_break) label="Break" ;;
  long_break) label="Long break" ;;
  *) exit 0 ;; # off or unreadable
esac

paused=$(get paused)
waiting=$(get waiting)
ends_at=$(get ends_at)

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

# The task name is a JSON string: take it up to the first unescaped quote, undo the escapes and
# shorten it. `#` is doubled because tmux would read `#[` or `#(` in the output as a style or command.
max=${PTC_TMUX_MAX_TASK:-30}
task=$(sed -n 's/.*"text": *"\(\([^"\\]\|\\.\)*\)".*/\1/p' "$file" | head -n1 \
  | sed -e 's/\\"/"/g' -e 's/\\\\/\\/g' -e 's/\\[nrt]/ /g' -e 's/#/##/g')
if [ "${#task}" -gt "$max" ]; then
  task="${task:0:$((max - 1))}…"
fi
[ -n "$task" ] && task=" · $task"

printf '🍅 %s %02d:%02d%s%s\n' "$label" $((left / 60)) $((left % 60)) "$suffix" "$task"
