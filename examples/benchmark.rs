//! Benchmark Example
//!
//! Run with:
//!   cargo run --example benchmark -- --help
//!
//! Examples:
//!   # Basic benchmark (1000 tasks, 10 workers)
//!   cargo run --release --example benchmark -- --tasks 1000 --workers 10
//!
//!   # High throughput test
//!   cargo run --release --example benchmark -- --tasks 100000 --workers 50 --batch-size 100
//!
//!   # Latency test
//!   cargo run --release --example benchmark -- --tasks 1000 --workers 10 --task-duration 1

use rediq::client::Client;
use rediq::processor::{Handler, Mux};
use rediq::server::{Server, ServerBuilder};
use rediq::Task;
use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Number of tasks to enqueue
    #[arg(short, long, default_value_t = 1000)]
    tasks: usize,

    /// Number of concurrent workers
    #[arg(short, long, default_value_t = 10)]
    workers: usize,

    /// Batch size for enqueueing
    #[arg(long, default_value_t = 10)]
    batch_size: usize,

    /// Simulated task duration in milliseconds
    #[arg(long, default_value_t = 10)]
    task_duration: u64,

    /// Number of queues
    #[arg(long, default_value_t = 1)]
    queues: usize,

    /// Use priority queue
    #[arg(long, default_value_t = false)]
    priority: bool,

    /// Task payload size in KB
    #[arg(long, default_value_t = 1)]
    payload_size: usize,

    /// Redis URL
    #[arg(long, default_value = "redis://localhost:6379")]
    redis_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct BenchmarkData {
    id: u64,
    payload: Vec<u8>,
    created_at: u64,
}

#[derive(Clone)]
struct BenchmarkHandler {
    duration: Duration,
    processed: Arc<AtomicU64>,
}

#[async_trait]
impl Handler for BenchmarkHandler {
    async fn handle(&self, _task: &rediq::Task) -> rediq::Result<()> {
        // Simulate work
        tokio::time::sleep(self.duration).await;

        self.processed.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize tracing (minimal for benchmark)
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                    Rediq Benchmark Tool                      ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("Configuration:");
    println!("  Tasks:          {}", args.tasks);
    println!("  Workers:        {}", args.workers);
    println!("  Batch size:     {}", args.batch_size);
    println!("  Task duration:  {}ms", args.task_duration);
    println!("  Queues:         {}", args.queues);
    println!("  Priority:       {}", args.priority);
    println!("  Payload size:   {}KB", args.payload_size);
    println!("  Redis URL:      {}", args.redis_url);
    println!();

    let processed = Arc::new(AtomicU64::new(0));
    let handler = BenchmarkHandler {
        duration: Duration::from_millis(args.task_duration),
        processed: processed.clone(),
    };

    // Build server state
    let queue_names: Vec<String> = (0..args.queues)
        .map(|i| format!("queue-{}", i))
        .collect();

    let state = ServerBuilder::new()
        .redis_url(&args.redis_url)
        .queues(&queue_names.iter().map(|s| s.as_str()).collect::<Vec<_>>())
        .concurrency(args.workers)
        .build()
        .await?;

    // Create server
    let server = Server::from(state);

    // Register handlers
    let mut mux = Mux::new();
    for queue in &queue_names {
        mux.handle(queue.as_str(), handler.clone());
    }

    // Start server in background
    let server_handle = tokio::spawn(async move {
        server.run(mux).await
    });

    // Wait for server to be ready
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Create client
    let client = Client::builder()
        .redis_url(&args.redis_url)
        .build()
        .await?;

    // Prepare payload
    let payload = vec![0u8; args.payload_size * 1024];

    // ===== ENQUEUE PHASE =====
    println!("▶ Enqueueing {} tasks...", args.tasks);
    let enqueue_start = Instant::now();

    let mut enqueue_errors = 0;

    for batch in (0..args.tasks).step_by(args.batch_size) {
        let batch_end = (batch + args.batch_size).min(args.tasks);
        let batch_count = batch_end - batch;

        let mut tasks = Vec::with_capacity(batch_count);

        for i in batch..batch_end {
            let queue_idx = i % args.queues;
            let queue = &queue_names[queue_idx];

            let data = BenchmarkData {
                id: i as u64,
                payload: payload.clone(),
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_millis() as u64,
            };

            let mut builder = Task::builder(queue.clone())
                .payload(&data)?
                .max_retry(3)
                .timeout(Duration::from_secs(30));

            if args.priority {
                let priority = (i % 100) as i32;
                builder = builder.priority(priority);
            }

            tasks.push(builder.build()?);
        }

        match client.enqueue_batch(tasks).await {
            Ok(_) => {
                if (batch_end / args.batch_size) % 10 == 0 {
                    print!("\r  Progress: {}/{}", batch_end, args.tasks);
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
            }
            Err(e) => {
                enqueue_errors += 1;
                eprintln!("\n  Batch enqueue error: {}", e);
            }
        }
    }

    let enqueue_duration = enqueue_start.elapsed();
    println!("\r  Enqueued: {}/{}                                ", args.tasks - enqueue_errors, args.tasks);
    println!("  Time:     {:.2}s", enqueue_duration.as_secs_f64());
    println!("  Throughput: {:.0} tasks/sec", args.tasks as f64 / enqueue_duration.as_secs_f64());
    println!();

    // ===== PROCESSING PHASE =====
    println!("⏳ Waiting for tasks to be processed...");
    let process_start = Instant::now();

    let last_processed = Arc::new(AtomicU64::new(0));
    let last_processed_clone = last_processed.clone();
    let processed_clone = processed.clone();

    // Progress reporter
    let progress_handle = tokio::spawn(async move {
        let mut last_count = 0u64;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let current = processed_clone.load(Ordering::Relaxed);
            let delta = current - last_count;

            if delta > 0 {
                print!(
                    "\r  Processed: {}/{} (+{}/sec)          ",
                    current, args.tasks, delta
                );
                use std::io::Write;
                std::io::stdout().flush().ok();
            }

            last_processed_clone.store(current, Ordering::Relaxed);
            last_count = current;

            if current >= args.tasks as u64 {
                break;
            }
        }
    });

    // Wait for all tasks to complete (with timeout)
    let timeout = Duration::from_secs(300); // 5 minutes max
    let check_interval = Duration::from_millis(100);

    loop {
        let current = processed.load(Ordering::Relaxed);
        if current >= args.tasks as u64 {
            break;
        }

        if process_start.elapsed() > timeout {
            eprintln!("\n⚠ Timeout waiting for tasks to complete!");
            break;
        }

        tokio::time::sleep(check_interval).await;
    }

    progress_handle.abort();

    let process_duration = process_start.elapsed();
    let final_processed = processed.load(Ordering::Relaxed);

    println!("\r  Processed: {}/{}                              ", final_processed, args.tasks);
    println!("  Time:        {:.2}s", process_duration.as_secs_f64());
    println!("  Throughput:  {:.0} tasks/sec", final_processed as f64 / process_duration.as_secs_f64());
    println!();

    // ===== SUMMARY =====
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║                         Summary                            ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Total tasks:        {}", args.tasks);
    println!("  Successfully:       {}", final_processed);
    println!("  Failed:             {}", args.tasks as u64 - final_processed);
    println!();
    println!("  Enqueue time:       {:.2}s", enqueue_duration.as_secs_f64());
    println!("  Process time:       {:.2}s", process_duration.as_secs_f64());
    println!("  Total time:         {:.2}s", (enqueue_duration + process_duration).as_secs_f64());
    println!();
    println!("  Enqueue throughput: {:.0} tasks/sec", args.tasks as f64 / enqueue_duration.as_secs_f64());
    println!("  Process throughput: {:.0} tasks/sec", final_processed as f64 / process_duration.as_secs_f64());
    println!("  Overall throughput: {:.0} tasks/sec", final_processed as f64 / (enqueue_duration + process_duration).as_secs_f64());
    println!();

    // Calculate percentiles if we had timing data
    println!("  Avg latency:        ~{}ms", args.task_duration + process_duration.as_millis() as u64 / final_processed.max(1) as u64);

    // Shutdown server
    println!("Shutting down...");
    server_handle.abort();

    Ok(())
}
