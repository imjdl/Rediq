//! Task progress extension trait
//!
//! Provides an extension trait for Task to access progress reporting
//! functionality during task execution.

use crate::progress::ProgressContext;
use std::cell::RefCell;

// Thread-local storage for the current task's progress context
//
// This is set by the Worker before executing a task handler and cleared
// after execution completes.
thread_local! {
    static PROGRESS_CONTEXT: RefCell<Option<ProgressContext>> = const { RefCell::new(None) };
}

/// Progress guard for automatic cleanup
///
/// This guard ensures that the progress context is properly cleaned up
/// when it goes out of scope, preventing memory leaks.
///
/// # Example
///
/// ```rust,no_run
/// use rediq::task::TaskProgressExt;
/// use rediq::Task;
///
/// async fn process_task(task: &Task) {
///     let _guard = task.progress_guard().unwrap();
///     // Progress context is automatically cleaned up when _guard goes out of scope
/// }
/// ```
pub struct ProgressGuard;

impl Drop for ProgressGuard {
    fn drop(&mut self) {
        clear_progress_context();
    }
}

/// Task progress extension trait
///
/// Allows tasks to access progress reporting functionality during execution.
pub trait TaskProgressExt {
    /// Get the progress reporter for this task
    ///
    /// Returns `None` if:
    /// - Progress tracking is not enabled
    /// - Called outside of task execution context
    /// - The context has already been cleaned up
    fn progress(&self) -> Option<ProgressContext>;

    /// Create a progress guard for automatic cleanup
    ///
    /// The returned guard will automatically clear the progress context
    /// when dropped, preventing memory leaks.
    ///
    /// Returns `None` if progress tracking is not enabled.
    fn progress_guard(&self) -> Option<ProgressGuard>;

    /// Set the progress context (internal use by Worker)
    #[doc(hidden)]
    fn _set_progress(&self, ctx: Option<ProgressContext>);

    /// Check if progress reporting is available
    fn has_progress(&self) -> bool;
}

impl TaskProgressExt for super::Task {
    fn progress(&self) -> Option<ProgressContext> {
        PROGRESS_CONTEXT.with(|cell| {
            cell.borrow().as_ref().and_then(|ctx| {
                // Verify task ID matches (security check)
                if ctx.task_id() == self.id {
                    Some(ctx.clone())
                } else {
                    None
                }
            })
        })
    }

    fn progress_guard(&self) -> Option<ProgressGuard> {
        // Only return a guard if progress context exists for this task
        PROGRESS_CONTEXT.with(|cell| {
            cell.borrow().as_ref().and_then(|ctx| {
                if ctx.task_id() == self.id {
                    Some(ProgressGuard)
                } else {
                    None
                }
            })
        })
    }

    fn _set_progress(&self, ctx: Option<ProgressContext>) {
        PROGRESS_CONTEXT.with(|cell| {
            *cell.borrow_mut() = ctx;
        });
    }

    fn has_progress(&self) -> bool {
        PROGRESS_CONTEXT.with(|cell| {
            cell.borrow().is_some()
        })
    }
}

/// Set the progress context for the current task
///
/// This is called by the Worker before executing a task handler.
///
/// # Important
///
/// When using this function, ensure the context is cleared by either:
/// 1. Using `progress_guard()` to create a guard with automatic cleanup
/// 2. Explicitly calling `clear_progress_context()` when done
/// 3. Setting to `None` after task execution completes
pub fn set_progress_context(ctx: Option<ProgressContext>) {
    PROGRESS_CONTEXT.with(|cell| {
        *cell.borrow_mut() = ctx;
    });
}

/// Get the current progress context (if any)
///
/// This is useful for middleware or other components that need
/// to access progress information.
pub fn get_progress_context() -> Option<ProgressContext> {
    PROGRESS_CONTEXT.with(|cell| {
        cell.borrow().clone()
    })
}

/// Clear the current progress context
///
/// This should be called after task execution completes to prevent
/// memory leaks. Using `progress_guard()` is preferred as it handles
/// cleanup automatically.
pub fn clear_progress_context() {
    set_progress_context(None);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_context_none_initially() {
        assert!(!get_progress_context().is_some());
    }

    #[test]
    fn test_set_get_progress_context() {
        // Initially no context
        assert!(get_progress_context().is_none());

        // Note: Full integration test requires Redis connection
        // This is just a basic unit test for the API
    }
}
