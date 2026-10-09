-- Neovim module that shows the ptc pomodoro in the statusline.
-- Reads the status file that ptc always writes (core plugin `core_status_file.lua`, see README).
--
-- Install: with a plugin manager that adds the `contrib/nvim` subdirectory of this repository
-- to the runtimepath (see README), or copy this file to lua/ptc.lua in your Neovim config. Then:
--
--   require("ptc").setup()
--
--   -- plain statusline:
--   vim.o.statusline = "%f %= %{v:lua.require'ptc'.status()}"
--   -- or lualine:
--   require("lualine").setup({ sections = { lualine_x = { require("ptc").status } } })
--
-- Requires Neovim 0.10+ (vim.uv). Neovim's LuaJIT is Lua 5.1, so no `//` or integer subtypes here.

local M = {}

local uv = vim.uv or vim.loop
local path = vim.env.PTC_STATUS_FILE
    or ((vim.env.XDG_RUNTIME_DIR or "/tmp") .. "/ptc-status.json")

local LABELS = {
    work = "Focus",
    short_break = "Break",
    long_break = "Long break",
}

local current = nil -- last status read from the file

local function read()
    local file = io.open(path, "r")
    if not file then
        current = nil
        return
    end
    local ok, decoded = pcall(vim.json.decode, file:read("*a"))
    file:close()
    current = ok and type(decoded) == "table" and decoded or nil
end

local function clock(seconds)
    if seconds >= 3600 then
        return string.format("%d:%02d:%02d", math.floor(seconds / 3600),
            math.floor(seconds % 3600 / 60), seconds % 60)
    end
    return string.format("%02d:%02d", math.floor(seconds / 60), seconds % 60)
end

--- Text for the statusline, or "" when there is nothing to show.
function M.status()
    local s = current
    -- Idle after a break that finished by itself: the app's "Idle for MM:SS". A plain idle (never
    -- started, or stopped by hand) has no `idle_since` and stays hidden.
    if s and s.state == "idle" and s.idle_since then
        return "🍅 Idle for " .. clock(math.max(0, os.time() - s.idle_since))
    end
    if not s or not LABELS[s.state] then
        return "" -- no ptc running, plain idle or quit
    end

    local counting = s.ends_at and not s.paused and not s.waiting
    local left
    if counting then
        -- A running phase far past its end means ptc died without saying goodbye.
        if os.time() > s.ends_at + 5 then
            return ""
        end
        left = math.max(0, s.ends_at - os.time())
    else
        left = s.remaining_seconds or 0
    end

    local suffix = ""
    if s.paused then
        suffix = " (paused)"
    elseif s.waiting then
        suffix = " (ready)"
    end
    return string.format("🍅 %s %02d:%02d%s", LABELS[s.state], math.floor(left / 60), left % 60, suffix)
end

--- Starts a timer that re-reads the file and redraws the statusline every second.
function M.setup()
    read()
    local timer = uv.new_timer()
    timer:start(1000, 1000, vim.schedule_wrap(function()
        read()
        vim.cmd("redrawstatus")
    end))
end

return M
