//! Utility functions for CLI commands
//!
//! Provides Redis URL parsing, command formatting, and helper functions.

/// Redis connection information extracted from URL
#[derive(Debug, Clone)]
pub struct RedisConnectionInfo {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub db: i64,
}

impl RedisConnectionInfo {
    /// Create a new connection info with default values
    pub fn localhost() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: 0,
        }
    }
}

impl Default for RedisConnectionInfo {
    fn default() -> Self {
        Self::localhost()
    }
}

/// Parse Redis URL and extract connection details
///
/// Supports formats:
/// - `redis://host:port/db`
/// - `redis://:password@host:port/db`
/// - `redis://host:port`
///
/// # Arguments
/// * `url` - Redis connection URL
///
/// # Returns
/// Parsed connection information
///
/// # Example
/// ```
/// use rediq_cli::utils::parse_redis_url;
///
/// let info = parse_redis_url("redis://localhost:6379/0");
/// assert_eq!(info.host, "localhost");
/// assert_eq!(info.port, 6379);
/// assert_eq!(info.db, 0);
/// ```
pub fn parse_redis_url(url: &str) -> RedisConnectionInfo {
    // Default values
    let mut host = "localhost".to_string();
    let mut port = 6379;
    let mut password = None;
    let mut db = 0;

    // Remove protocol prefix
    let url = url.strip_prefix("redis://").unwrap_or(url);

    // Parse format: [:password@]host[:port][/db]
    let parts: Vec<&str> = url.split('@').collect();

    if parts.len() > 1 {
        // Has authentication: [password@]host[:port][/db]
        let auth_part = parts[0];
        let remaining = parts[1];

        // Check if password exists (format: :password or just @ for no password)
        if !auth_part.is_empty() {
            password = Some(auth_part.strip_prefix(':').unwrap_or(auth_part).to_string());
        }

        // Parse host:port/db from remaining part
        parse_host_port_db(remaining, &mut host, &mut port, &mut db);
    } else {
        // No authentication: host[:port][/db]
        parse_host_port_db(parts[0], &mut host, &mut port, &mut db);
    }

    RedisConnectionInfo { host, port, password, db }
}

/// Parse host, port, and database from URL part
///
/// # Arguments
/// * `s` - String containing host[:port][/db]
/// * `host` - Output parameter for host
/// * `port` - Output parameter for port
/// * `db` - Output parameter for database number
fn parse_host_port_db(s: &str, host: &mut String, port: &mut u16, db: &mut i64) {
    // Split by / to separate [host:port] and [db]
    let parts: Vec<&str> = s.split('/').collect();

    let addr_part = parts[0];

    // Parse host:port
    if let Some(colon_pos) = addr_part.rfind(':') {
        if let Some(parsed_port) = addr_part[colon_pos + 1..].parse::<u16>().ok() {
            *port = parsed_port;
            *host = addr_part[..colon_pos].to_string();
        } else {
            *host = addr_part.to_string();
        }
    } else {
        *host = addr_part.to_string();
    }

    // Parse database number
    if parts.len() > 1 {
        if let Some(parsed_db) = parts[1].parse::<i64>().ok() {
            *db = parsed_db;
        }
    }
}

/// Build redis-cli arguments from connection info
///
/// # Arguments
/// * `info` - Redis connection information
///
/// # Returns
/// String containing redis-cli command line arguments
///
/// # Example
/// ```
/// use rediq_cli::utils::{build_redis_cli_args, RedisConnectionInfo};
///
/// let info = RedisConnectionInfo {
///     host: "localhost".to_string(),
///     port: 6379,
///     password: Some("secret".to_string()),
///     db: 0,
/// };
/// let args = build_redis_cli_args(&info);
/// assert!(args.contains("-h localhost"));
/// assert!(args.contains("-p 6379"));
/// ```
pub fn build_redis_cli_args(info: &RedisConnectionInfo) -> String {
    let mut args = vec![
        format!("-h {}", info.host),
        format!("-p {}", info.port),
    ];

    if let Some(ref pwd) = info.password {
        args.push(format!("-a \"{}\"", pwd));
    }

    args.push("--no-auth-warning".to_string());

    if info.db > 0 {
        args.push(format!("-n {}", info.db));
    }

    args.join(" ")
}

/// Print a redis-cli command with connection info
///
/// # Arguments
/// * `info` - Redis connection information
/// * `cmd` - Redis command to execute
pub fn print_redis_command(info: &RedisConnectionInfo, cmd: &str) {
    println!("  redis-cli {} {}", build_redis_cli_args(info), cmd);
}

/// Print a redis-cli command with connection info and indentation
///
/// # Arguments
/// * `info` - Redis connection information
/// * `indent` - Number of spaces to indent
/// * `cmd` - Redis command to execute
pub fn print_redis_command_indented(info: &RedisConnectionInfo, indent: usize, cmd: &str) {
    let indent_str = " ".repeat(indent);
    println!("{}redis-cli {} {}", indent_str, build_redis_cli_args(info), cmd);
}

/// Shorten a string to a maximum length
///
/// If the string is longer than `max_len`, it will be truncated
/// with "..." appended. Otherwise, the original string is returned.
///
/// # Arguments
/// * `s` - String to shorten
/// * `max_len` - Maximum length
///
/// # Returns
/// Shortened string
///
/// # Example
/// ```
/// use rediq_cli::utils::shorten_string;
///
/// assert_eq!(shorten_string("hello", 10), "hello");
/// assert_eq!(shorten_string("hello world", 8), "hello...");
/// ```
#[allow(dead_code)]
pub fn shorten_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Shorten an ID to a maximum length
///
/// Keeps the end of the ID visible by prefixing with "..."
///
/// # Arguments
/// * `id` - ID string to shorten
/// * `max_len` - Maximum length
///
/// # Returns
/// Shortened ID with suffix preserved
///
/// # Example
/// ```
/// use rediq_cli::utils::shorten_id;
///
/// let long_id = "abc123def456ghi789jkl012mno345pqr678stu901vwx234yz";
/// assert!(shorten_id(&long_id, 20).starts_with("..."));
/// assert!(shorten_id(&long_id, 20).ends_with("vwx234yz"));
/// ```
#[allow(dead_code)]
pub fn shorten_id(id: &str, max_len: usize) -> String {
    if id.len() <= max_len {
        id.to_string()
    } else {
        format!("...{}", &id[id.len().saturating_sub(max_len - 3)..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_redis_url_basic() {
        let info = parse_redis_url("redis://localhost:6379");
        assert_eq!(info.host, "localhost");
        assert_eq!(info.port, 6379);
        assert_eq!(info.db, 0);
        assert!(info.password.is_none());
    }

    #[test]
    fn test_parse_redis_url_with_db() {
        let info = parse_redis_url("redis://localhost:6379/2");
        assert_eq!(info.host, "localhost");
        assert_eq!(info.port, 6379);
        assert_eq!(info.db, 2);
    }

    #[test]
    fn test_parse_redis_url_with_password() {
        let info = parse_redis_url("redis://:secret@localhost:6379");
        assert_eq!(info.host, "localhost");
        assert_eq!(info.port, 6379);
        assert_eq!(info.password, Some("secret".to_string()));
    }

    #[test]
    fn test_parse_redis_url_with_password_and_db() {
        let info = parse_redis_url("redis://:secret@localhost:6379/1");
        assert_eq!(info.host, "localhost");
        assert_eq!(info.port, 6379);
        assert_eq!(info.password, Some("secret".to_string()));
        assert_eq!(info.db, 1);
    }

    #[test]
    fn test_parse_redis_url_custom_host() {
        let info = parse_redis_url("redis://redis.example.com:6380/3");
        assert_eq!(info.host, "redis.example.com");
        assert_eq!(info.port, 6380);
        assert_eq!(info.db, 3);
    }

    #[test]
    fn test_build_redis_cli_args_basic() {
        let info = RedisConnectionInfo {
            host: "localhost".to_string(),
            port: 6379,
            password: None,
            db: 0,
        };
        let args = build_redis_cli_args(&info);
        assert!(args.contains("-h localhost"));
        assert!(args.contains("-p 6379"));
        assert!(args.contains("--no-auth-warning"));
    }

    #[test]
    fn test_build_redis_cli_args_with_password() {
        let info = RedisConnectionInfo {
            host: "redis.example.com".to_string(),
            port: 6380,
            password: Some("secret123".to_string()),
            db: 2,
        };
        let args = build_redis_cli_args(&info);
        assert!(args.contains("-h redis.example.com"));
        assert!(args.contains("-p 6380"));
        assert!(args.contains("-a \"secret123\""));
        assert!(args.contains("-n 2"));
    }

    #[test]
    fn test_shorten_string() {
        assert_eq!(shorten_string("hello", 10), "hello");
        assert_eq!(shorten_string("hello world", 8), "hello...");
        assert_eq!(shorten_string("hi", 5), "hi");
    }

    #[test]
    fn test_shorten_id() {
        let long_id = "abc123def456ghi789jkl012mno345pqr678stu901vwx234yz";
        let shortened = shorten_id(&long_id, 20);
        assert!(shortened.starts_with("..."));
        assert!(shortened.ends_with("vwx234yz"));
        assert!(shortened.len() <= 20);
    }
}
