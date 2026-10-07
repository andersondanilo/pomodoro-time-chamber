-- Core plugin: publishes the pomodoro status to a JSON file so other programs
-- (a Neovim statusline, a tmux/polybar/waybar script, ...) can show it.
--
-- File: `ptc.status_file` ($PTC_STATUS_FILE, or $XDG_RUNTIME_DIR/ptc-status.json, falling back to /tmp)
--
-- The file holds: { state, paused, waiting, ends_at, remaining_seconds,
--                   completed_pomodoros, updated_at }
-- `state` is "idle", "work", "short_break", "long_break", or "off" (the app has quit).
-- While counting down, readers compute the time left as `ends_at - now`; when `ends_at`
-- is absent (paused, waiting, idle) they use `remaining_seconds` as is.

local path = ptc.status_file

local function write(status)
    status.updated_at = os.time()
    -- Temp file + rename, so readers never see a half-written file.
    local tmp = path .. ".tmp"
    local file = assert(io.open(tmp, "w"))
    file:write(ptc.json.encode(status))
    file:close()
    assert(os.rename(tmp, path))
end

ptc.on("startup", function()
    write({ state = "idle", paused = false, waiting = false })
end)

ptc.on("pomodoro_state_changed", function(e)
    write({
        state = e.state,
        paused = e.paused,
        waiting = e.waiting,
        ends_at = e.ends_at,
        remaining_seconds = e.remaining_seconds,
        completed_pomodoros = e.completed_pomodoros,
    })
end)

ptc.on("quit", function()
    write({ state = "off" })
end)
