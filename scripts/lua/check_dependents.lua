-- rediq_check_dependents.lua
-- Atomic dependency check and enqueue script
--
-- This script is called when a task completes. It checks if any tasks were waiting
-- for this task and enqueues them if all their dependencies are satisfied.
--
-- KEYS[1]: rediq:task_deps:{completed_task_id} - Set of dependent task IDs
--
-- ARGV[1]: completed_task_id - The ID of the completed task
-- ARGV[2]: current_timestamp - Current Unix timestamp
-- ARGV[3]: task_ttl - TTL for task details in seconds

local task_deps_key = KEYS[1]
local completed_task_id = ARGV[1]
local current_timestamp = ARGV[2]
local task_ttl = tonumber(ARGV[3]) or 86400

-- Get all tasks that depend on the completed task
local dependents = redis.call('SMEMBERS', task_deps_key)

if #dependents == 0 then
    -- No dependent tasks, clean up and return
    redis.call('DEL', task_deps_key)
    return {ok = 0, enqueued = {}}
end

local enqueued = {}
local failed = {}

-- Process each dependent task
for _, dependent_id in ipairs(dependents) do
    local pending_deps_key = 'rediq:pending_deps:' .. dependent_id

    -- Remove the completed task from pending dependencies
    redis.call('SREM', pending_deps_key, completed_task_id)

    -- Check if all dependencies are satisfied
    local remaining = redis.call('SCARD', pending_deps_key)

    if remaining == 0 then
        -- All dependencies satisfied, get task details
        local task_key = 'rediq:task:' .. dependent_id

        -- Check if task exists
        if redis.call('EXISTS', task_key) == 1 then
            -- Get queue name from task (stored in 'queue' field during enqueue)
            -- Note: This assumes enqueue.lua stores queue info
            local queue_info = redis.call('HGET', task_key, 'queue')

            if queue_info then
                -- Enqueue the task
                local queue_key = 'rediq:queue:' .. queue_info
                redis.call('RPUSH', queue_key, dependent_id)

                -- Update task status
                redis.call('HSET', task_key, 'status', 'pending')
                redis.call('HSET', task_key, 'enqueued_at', current_timestamp)
                redis.call('EXPIRE', task_key, task_ttl)

                -- Clean up pending deps
                redis.call('DEL', pending_deps_key)

                table.insert(enqueued, dependent_id)
            else
                table.insert(failed, dependent_id)
            end
        else
            table.insert(failed, dependent_id)
        end
    end
end

-- Clean up the task deps key for the completed task
redis.call('DEL', task_deps_key)

return {ok = #enqueued, enqueued = enqueued, failed = failed}
