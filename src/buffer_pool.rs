//! Buffer pooling for efficient memory reuse
//!
//! This module provides a buffer pool to reduce allocation overhead during network tests.
//! Buffers are pre-allocated and reused across multiple send/receive operations.

use std::sync::Mutex;

/// A thread-safe pool of reusable byte buffers
///
/// Reduces allocation overhead by reusing buffers across multiple I/O operations.
/// Particularly beneficial for UDP tests with many small packets and high-throughput TCP tests.
pub struct BufferPool {
    pool: Mutex<Vec<Vec<u8>>>,
    buffer_size: usize,
    max_pool_size: usize,
}

impl BufferPool {
    /// Creates a new buffer pool
    ///
    /// # Arguments
    ///
    /// * `buffer_size` - Size of each buffer in bytes
    /// * `max_pool_size` - Maximum number of buffers to keep in the pool
    pub fn new(buffer_size: usize, max_pool_size: usize) -> Self {
        Self {
            pool: Mutex::new(Vec::with_capacity(max_pool_size)),
            buffer_size,
            max_pool_size,
        }
    }

    /// Gets a buffer from the pool or allocates a new one
    ///
    /// Returns a zeroed buffer of the configured size. If the pool is empty,
    /// a new buffer is allocated. Otherwise, a buffer is reused from the pool.
    pub fn get(&self) -> Vec<u8> {
        let mut pool = self.pool.lock().unwrap();
        pool.pop().unwrap_or_else(|| vec![0u8; self.buffer_size])
    }

    /// Returns a buffer to the pool for reuse
    ///
    /// The buffer is cleared (zeroed) and added back to the pool if there's space.
    /// If the pool is at capacity, the buffer is dropped.
    pub fn put(&self, mut buffer: Vec<u8>) {
        // Only return buffers of the correct size
        if buffer.len() != self.buffer_size {
            return;
        }

        let mut pool = self.pool.lock().unwrap();
        if pool.len() < self.max_pool_size {
            // Clear the buffer for reuse
            buffer.fill(0);
            pool.push(buffer);
        }
        // Otherwise, let the buffer be dropped
    }

    /// Pre-allocates buffers to fill the pool
    ///
    /// Useful for warming up the pool before starting a test to avoid
    /// allocations during the test run.
    pub fn preallocate(&self) {
        let mut pool = self.pool.lock().unwrap();
        let current_size = pool.len();
        
        for _ in current_size..self.max_pool_size {
            pool.push(vec![0u8; self.buffer_size]);
        }
    }

    /// Returns the current number of buffers in the pool
    pub fn size(&self) -> usize {
        self.pool.lock().unwrap().len()
    }

    /// Clears all buffers from the pool
    pub fn clear(&self) {
        self.pool.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_pool_basic() {
        let pool = BufferPool::new(1024, 10);
        
        // Get a buffer
        let buf1 = pool.get();
        assert_eq!(buf1.len(), 1024);
        assert_eq!(pool.size(), 0);
        
        // Return it
        pool.put(buf1);
        assert_eq!(pool.size(), 1);
        
        // Get it again (should reuse)
        let buf2 = pool.get();
        assert_eq!(buf2.len(), 1024);
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_buffer_pool_max_size() {
        let pool = BufferPool::new(1024, 3);
        
        // Add 5 buffers, but only 3 should be kept
        let mut buffers = Vec::new();
        for _ in 0..5 {
            buffers.push(pool.get());
        }
        
        // Return all buffers
        for buf in buffers {
            pool.put(buf);
        }
        
        assert_eq!(pool.size(), 3);
    }

    #[test]
    fn test_buffer_pool_preallocate() {
        let pool = BufferPool::new(1024, 5);
        assert_eq!(pool.size(), 0);
        
        pool.preallocate();
        assert_eq!(pool.size(), 5);
    }

    #[test]
    fn test_buffer_pool_wrong_size() {
        let pool = BufferPool::new(1024, 3);
        
        // Try to return a buffer of wrong size
        let wrong_buf = vec![0u8; 2048];
        pool.put(wrong_buf);
        
        assert_eq!(pool.size(), 0);
    }

    #[test]
    fn test_buffer_pool_clear() {
        let pool = BufferPool::new(1024, 5);
        pool.preallocate();
        assert_eq!(pool.size(), 5);
        
        pool.clear();
        assert_eq!(pool.size(), 0);
    }
}
