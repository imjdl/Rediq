-- rediq_ack.lua
-- Atomic acknowledge script
--
-- KEYS[1]: rediq:active:{queue_name}
-- KEYS[2]: rediq:task:{task_id}
-- KEYS[3]: rediq:dedup:{queue_name}
-- KEYS[4]: rediq:stats:{queue_name}
--
-- ARGV[1]: task_id
-- ARGV[2]: unique_key (optional)
-- ARGV[3]: final status (processed/dead)
-- ARGV[4]: error_message (optional)
-- ARGV[5]: task_ttl (optional, TTL in seconds for task details, default 86400)

local active_key = KEYS[1]
local task_key = KEYS[2]
local dedup_key = KEYS[3]
local stats_key = KEYS[4]
local task_id = ARGV[1]
local unique_key = ARGV[2]
local status = ARGV[3]
local error_msg = ARGV[4]
local task_ttl = tonumber(ARGV[5]) or 86400

-- Remove from active queue
redis.call('LREM', active_key, 1, task_id)

-- Update task status
redis.call('HSET', task_key, 'status', status)
if error_msg and error_msg ~= '' then
    redis.call('HSET', task_key, 'last_error', error_msg)
end
redis.call('EXPIRE', task_key, task_ttl)

-- If successful, remove deduplication record
if status == 'processed' and unique_key and unique_key ~= '' then
    redis.call('SREM', dedup_key, unique_key)
end

-- Update statistics
if status == 'processed' then
    redis.call('HINCRBY', stats_key, 'processed', 1)
elseif status == 'dead' then
    redis.call('HINCRBY', stats_key, 'dead', 1)
end
redis.call('HINCRBY', stats_key, 'total', 1)

return {ok = status}
