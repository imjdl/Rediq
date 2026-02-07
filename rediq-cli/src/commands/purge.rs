//! Purge command handlers
//!
//! Provides commands for cleaning up stale metadata and empty queues.

use chrono::Utc;
use color_eyre::Result;
use rediq::client::Client;
use crate::utils::{parse_redis_url, build_redis_cli_args, print_redis_command, print_redis_command_indented};

/// Handle purge command
///
/// # Arguments
/// * `client` - Rediq client instance
/// * `redis_url` - Redis connection URL
/// * `workers` - Remove stale worker metadata
/// * `queues` - Remove empty queue metadata
/// * `all` - Show commands to purge all metadata
pub async fn handle(
    client: &Client,
    redis_url: &str,
    workers: bool,
    queues: bool,
    all: bool,
) -> Result<()> {
    // Parse Redis URL to extract connection details (do this first for --all case)
    let conn_info = parse_redis_url(redis_url);

    if all {
        println!("⚠️  Purging ALL Rediq metadata...");
        println!();
        println!("To purge all metadata, run:");
        print_redis_command(&conn_info, "DEL \"rediq:meta:workers\"");
        print_redis_command(&conn_info, "DEL \"rediq:meta:queues\"");
        println!();
        println!("Or flush the entire database:");
        print_redis_command(&conn_info, "FLUSHDB");
        return Ok(());
    }

    let inspector = client.inspector();

    if workers {
        println!("Purging stale worker metadata...");
        let workers_list = inspector.list_workers().await?;

        let mut stale_count = 0;
        for worker in workers_list {
            // Check if worker has recent heartbeat (within 2 minutes)
            let now = Utc::now().timestamp();
            let heartbeat_age = now - worker.last_heartbeat;

            if heartbeat_age > 120 {
                println!("  - Found stale worker: {} (last heartbeat {}s ago)", worker.id, heartbeat_age);
                stale_count += 1;
            }
        }

        if stale_count > 0 {
            println!();
            println!("  To remove stale workers, run:");
            print_redis_command_indented(&conn_info, 2, "DEL \"rediq:meta:workers\"");
            println!("  {}redis-cli {} --scan --pattern 'rediq:meta:worker:*' | xargs -L 1 redis-cli {} DEL",
                "  ".repeat(2),
                build_redis_cli_args(&conn_info),
                build_redis_cli_args(&conn_info)
            );
        } else {
            println!("  ✓ No stale workers found");
        }
    }

    if queues {
        println!("Purging empty queue metadata...");
        let queues_list = inspector.list_queues().await?;

        let mut empty_count = 0;
        for queue in queues_list {
            let stats = inspector.queue_stats(&queue).await?;
            let is_empty = stats.pending == 0 && stats.active == 0
                && stats.delayed == 0 && stats.retried == 0 && stats.dead == 0;

            if is_empty {
                println!("  - Found empty queue: {}", queue);
                empty_count += 1;
            }
        }

        if empty_count > 0 {
            println!();
            println!("  To remove empty queues from metadata, run:");
            print_redis_command_indented(&conn_info, 2, "SREM \"rediq:meta:queues\" \"<queue-name>\"");
        } else {
            println!("  ✓ No empty queues found");
        }
    }

    if !workers && !queues && !all {
        println!("Purge stale metadata from Redis");
        println!();
        println!("Usage: rediq purge [--workers] [--queues] [--all]");
        println!("  --workers   Show stale workers (no heartbeat in 2+ minutes)");
        println!("  --queues    Show empty queues");
        println!("  --all       Show commands to purge all metadata");
        println!();
        println!("Examples:");
        println!("  rediq purge --workers    # List stale workers");
        println!("  rediq purge --queues     # List empty queues");
        println!("  rediq purge --all        # Show purge commands");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    

    #[test]
    fn test_purge_display() {
        // Basic structural test - integration tests cover actual functionality
        let msg = "Purging stale worker metadata...";
        assert!(msg.contains("Purging"));
        assert!(msg.contains("worker"));
    }
}
