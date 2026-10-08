-- Sets up the event system of the `ptc` global. Runs once, before any plugin.
-- Returns the `emit(event, payload)` function used by the host.

local EVENTS = {
    startup = true,                -- once, before the first draw; payload: nil
    quit = true,                   -- once, when the app exits; payload: nil
    tasks_changed = true,          -- payload: list of tasks
    pomodoro_state_changed = true, -- payload: { state, previous, paused, waiting, completed_pomodoros,
                                   --   remaining_seconds, ends_at?, current_task?, changed }
}

local handlers = {}

function ptc.on(event, handler)
    if not EVENTS[event] then
        error("unknown event: " .. tostring(event), 2)
    end
    if type(handler) ~= "function" then
        error("handler must be a function", 2)
    end
    handlers[event] = handlers[event] or {}
    table.insert(handlers[event], handler)
end

return function(event, payload)
    for _, handler in ipairs(handlers[event] or {}) do
        local ok, err = pcall(handler, payload)
        if not ok then
            ptc._report(event .. " handler failed: " .. tostring(err))
        end
    end
end
