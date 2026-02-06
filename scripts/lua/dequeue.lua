-- rediq_dequeue.lua
-- Atomic dequeue script
--
-- KEYS[1]: rediq:queue:{queue_name}
-- KEYS[2]: rediq:active:{queue_name}
-- KEYS[3]: rediq:pause:{queue_name}
--
-- ARGV[1]: timeout (seconds)

local queue_key = KEYS[1]
local active_key = KEYS[2]
local pause_key = KEYS[3]
local timeout = tonumber(ARGV[1])

-- Check if queue is paused
if redis.call('EXISTS', pause_key) == 1 then
    return {err = 'ERR_QUEUE_PAUSED'}
end

-- Blocking dequeue
local result = redis.call('BLPOP', queue_key, timeout)
if not result or #result == 0 then
    return {err = 'ERR_TIMEOUT'}
end

local task_id = result[2]

-- Move to active queue
redis.call('LPUSH', active_key, task_id)

-- Update task status
local task_key = 'rediq:task:' .. task_id
redis.call('HSET', task_key, 'status', 'active')
redis.call('HSET', task_key, 'processed_at', ARGV[2])
redis.call('EXPIRE', task_key, 86400)

return {ok = task_id}
