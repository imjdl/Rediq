-- rediq_penqueue.lua
-- Priority enqueue script
--
-- KEYS[1]: rediq:pqueue:{queue_name} (ZSet)
-- KEYS[2]: rediq:dedup:{queue_name}
-- KEYS[3]: rediq:task:{task_id}
--
-- ARGV[1]: task_id
-- ARGV[2]: unique_key (optional, empty string means disabled)
-- ARGV[3]: task_data (serialized task data)
-- ARGV[4]: priority (0-100, lower value = higher priority)
-- ARGV[5]: task_ttl (optional, TTL in seconds for task details, default 86400)

local pqueue_key = KEYS[1]
local dedup_key = KEYS[2]
local task_key = KEYS[3]
local task_id = ARGV[1]
local unique_key = ARGV[2]
local task_data = ARGV[3]
local priority = tonumber(ARGV[4])
local task_ttl = tonumber(ARGV[5]) or 86400

-- Validate priority
if priority < 0 or priority > 100 then
    return {err = 'ERR_INVALID_PRIORITY'}
end

-- Deduplication check
if unique_key and unique_key ~= '' then
    local exists = redis.call('SISMEMBER', dedup_key, unique_key)
    if exists == 1 then
        return {err = 'ERR_DUPLICATE_TASK'}
    end
    redis.call('SADD', dedup_key, unique_key)
end

-- Check if task already exists
if redis.call('EXISTS', task_key) == 1 then
    return {err = 'ERR_TASK_EXISTS'}
end

-- Store task details
redis.call('HSET', task_key, 'data', task_data)
redis.call('HSET', task_key, 'priority', priority)
redis.call('EXPIRE', task_key, task_ttl)

-- Add task_id to priority queue (lower score = higher priority)
redis.call('ZADD', pqueue_key, priority, task_id)

-- Register queue
redis.call('SADD', 'rediq:meta:queues', pqueue_key:match('rediq:pqueue:(.+)'))

return {ok = task_id}
