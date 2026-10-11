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
    -- Written through a temp file, so a crash never leaves a half-written list.
    ptc.write_file(path, ptc.json.encode(tasks))
end)
