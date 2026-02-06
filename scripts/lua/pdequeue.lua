-- rediq_pdequeue.lua
-- Priority dequeue script
--
-- KEYS[1]: rediq:pqueue:{queue_name} (ZSet)
-- KEYS[2]: rediq:active:{queue_name}
-- KEYS[3]: rediq:pause:{queue_name}
--
-- ARGV[1]: timeout (seconds)
-- ARGV[2]: current_timestamp (Unix timestamp)

local pqueue_key = KEYS[1]
local active_key = KEYS[2]
local pause_key = KEYS[3]
local timeout = tonumber(ARGV[1])
local current_timestamp = tonumber(ARGV[2])

-- Check if queue is paused
if redis.call('EXISTS', pause_key) == 1 then
    return {err = 'ERR_QUEUE_PAUSED'}
end

-- Get task with highest priority (lowest score)
-- Use ZRANGE with score 0 to get the first element
local results = redis.call('ZRANGE', pqueue_key, 0, 0)
if not results or #results == 0 then
    return {err = 'ERR_TIMEOUT'}
end

local task_id = results[1]

-- Remove from priority queue
redis.call('ZREM', pqueue_key, task_id)

-- Move to active queue
redis.call('LPUSH', active_key, task_id)

-- Update task status
local task_key = 'rediq:task:' .. task_id
redis.call('HSET', task_key, 'status', 'active')
redis.call('HSET', task_key, 'processed_at', current_timestamp)
redis.call('EXPIRE', task_key, 86400)

return {ok = task_id}
