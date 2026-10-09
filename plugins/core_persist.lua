-- Core plugin: keeps the task list on disk (<data_dir>/tasks.json, or tasks.<preset>.json when
-- ptc runs with --preset, so each preset has its own tasks).

local path = ptc.data_dir .. "/tasks" .. (ptc.preset and ("." .. ptc.preset) or "") .. ".json"

ptc.on("startup", function()
    local file = io.open(path, "r")
    if not file then
        return -- first run
    end
    local content = file:read("a")
    file:close()
    ptc.set_tasks(ptc.json.decode(content))
end)

ptc.on("tasks_changed", function(tasks)
    -- Write to a temp file first so a crash never leaves a half-written list.
    local tmp = path .. ".tmp"
    local file = assert(io.open(tmp, "w"))
    file:write(ptc.json.encode(tasks))
    file:close()
    assert(os.rename(tmp, path))
end)
