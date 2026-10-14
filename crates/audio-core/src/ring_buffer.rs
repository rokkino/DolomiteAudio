//! Wait-Free Lockless Single-Producer Single-Consumer (SPSC) Ring Buffer
//! Designed for real-time Kernel/User audio streaming with ZERO allocations,
//! ZERO intermediate copies, and ZERO cache-line false sharing.
//!
//! # Safety and Concurrency Architecture:
//! - Producer and Consumer operate on separate 64-byte cache lines to eliminate
//!   hardware cache coherency ping-pong (False Sharing).
//! - All index synchronizations use explicit `Acquire` and `Release` atomic operations.
//! - Ring size is strictly a power of two, replacing expensive modulo division with
//!   bitwise AND operations (`index & (CAPACITY - 1)`), executing in 1 CPU cycle.
//! - Direct slice operations allow zero-copy mapping directly into hardware DMA buffers.

use core::sync::atomic::{AtomicU64, AtomicU32, Ordering};

/// Standard CPU cache line size for modern x86_64 architectures (Intel / AMD).
pub const CACHE_LINE_SIZE: usize = 64;

/// Align data structure to 64 bytes to guarantee cache-line isolation.
#[repr(align(64))]
#[derive(Debug)]
pub struct CachePadded<T>(pub T);

impl<T> CachePadded<T> {
    #[inline(always)]
    pub const fn new(val: T) -> Self {
        Self(val)
    }
}

impl<T> core::ops::Deref for CachePadded<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> core::ops::DerefMut for CachePadded<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Maximum number of discrete audio channels supported in high-end studio workflows
/// (Stereo, 5.1, 7.1, 7.1.4 Dolby Atmos, 22.2 Multichannel, and 64-channel MADI/Dante).
pub const MAX_AUDIO_CHANNELS: usize = 64;

/// An uncompressed multi-channel audio frame aligned to 64 bytes.
#[repr(C, align(64))]
#[derive(Copy, Clone, Debug)]
pub struct AudioFrame {
    /// 64 channels of 32-bit floating-point samples (normalized to [-1.0, 1.0])
    pub samples: [f32; MAX_AUDIO_CHANNELS],
    /// High-precision hardware timestamp (CPU cycle counter via RDTSC or QPC 100ns ticks)
    pub timestamp_cycles: u64,
    /// Monotonically increasing frame sequence counter for dropout and xrun tracking
    pub frame_id: u64,
    /// Bitmask indicating active channels in this frame
    pub channel_mask: u64,
    /// Reserved alignment padding for hardware DMA bus alignment
    pub _reserved: [u32; 2],
}

impl AudioFrame {
    pub const EMPTY: Self = Self {
        samples: [0.0; MAX_AUDIO_CHANNELS],
        timestamp_cycles: 0,
        frame_id: 0,
        channel_mask: 0x3, // Default stereo active
        _reserved: [0, 0],
    };

    #[inline(always)]
    pub fn new_stereo(left: f32, right: f32, frame_id: u64, timestamp: u64) -> Self {
        let mut frame = Self::EMPTY;
        frame.samples[0] = left;
        frame.samples[1] = right;
        frame.frame_id = frame_id;
        frame.timestamp_cycles = timestamp;
        frame.channel_mask = 0x3;
        frame
    }
}

/// SPSC Ring buffer capacity: 8192 frames.
/// At 192 kHz sample rate, this provides ~42.6 ms of jitter headroom.
/// At 48 kHz, this provides ~170.6 ms.
/// For true "bufferless / single-sample" operation, quantum sizes of 1 to 16 frames are used.
pub const RING_CAPACITY: usize = 8192;
pub const RING_MASK: u64 = (RING_CAPACITY as u64) - 1;

/// Compile-time verification that RING_CAPACITY is a power of two.
const _: () = assert!((RING_CAPACITY & (RING_CAPACITY - 1)) == 0, "RING_CAPACITY must be a power of two");

/// Shared-memory layout of the Lockless Wait-Free SPSC Ring Buffer.
/// Both Kernel (.sys) and User-Mode ASIO DLL / Native client map this structure.
#[repr(C)]
pub struct SpscRingBuffer {
    /// Write head: Only modified by the Producer thread (Kernel DMA Capture or User ASIO Output).
    /// Isolated on its own 64-byte cache line.
    pub write_cursor: CachePadded<AtomicU64>,

    /// Read tail: Only modified by the Consumer thread (User ASIO Input or Kernel DMA Playback).
    /// Isolated on its own 64-byte cache line.
    pub read_cursor: CachePadded<AtomicU64>,

    /// Local cached shadow cursors to minimize cross-core atomic bus snooping traffic
    pub cached_read_cursor: CachePadded<AtomicU64>,
    pub cached_write_cursor: CachePadded<AtomicU64>,

    /// Total overflow / xrun counter detected on write
    pub overrun_counter: CachePadded<AtomicU32>,

    /// Total underflow / xrun counter detected on read
    pub underrun_counter: CachePadded<AtomicU32>,

    /// Continuous array of audio frames aligned to 64 bytes
    pub frames: [AudioFrame; RING_CAPACITY],
}

impl SpscRingBuffer {
    /// Create a zero-initialized ring buffer
    pub const fn new() -> Self {
        Self {
            write_cursor: CachePadded::new(AtomicU64::new(0)),
            read_cursor: CachePadded::new(AtomicU64::new(0)),
            cached_read_cursor: CachePadded::new(AtomicU64::new(0)),
            cached_write_cursor: CachePadded::new(AtomicU64::new(0)),
            overrun_counter: CachePadded::new(AtomicU32::new(0)),
            underrun_counter: CachePadded::new(AtomicU32::new(0)),
            frames: [AudioFrame::EMPTY; RING_CAPACITY],
        }
    }

    #[cfg(feature = "std")]
    pub fn heap_boxed() -> std::boxed::Box<Self> {
        unsafe {
            let layout = std::alloc::Layout::new::<Self>();
            let ptr = std::alloc::alloc_zeroed(layout) as *mut Self;
            if ptr.is_null() {
                std::alloc::handle_alloc_error(layout);
            }
            std::boxed::Box::from_raw(ptr)
        }
    }

    /// Reset all cursors and counters to zero
    #[inline(always)]
    pub fn reset(&self) {
        self.write_cursor.store(0, Ordering::Relaxed);
        self.read_cursor.store(0, Ordering::Relaxed);
        self.cached_read_cursor.store(0, Ordering::Relaxed);
        self.cached_write_cursor.store(0, Ordering::Relaxed);
        self.overrun_counter.store(0, Ordering::Relaxed);
        self.underrun_counter.store(0, Ordering::Relaxed);
    }

    /// Number of frames currently available to be read by the consumer.
    #[inline(always)]
    pub fn available_read(&self) -> usize {
        let write = self.write_cursor.load(Ordering::Acquire);
        let read = self.read_cursor.load(Ordering::Relaxed);
        write.wrapping_sub(read) as usize
    }

    /// Number of frames available for writing without overwriting unread data.
    #[inline(always)]
    pub fn available_write(&self) -> usize {
        let write = self.write_cursor.load(Ordering::Relaxed);
        let read = self.read_cursor.load(Ordering::Acquire);
        RING_CAPACITY - (write.wrapping_sub(read) as usize)
    }

    /// Write a single audio frame (Wait-Free, zero-allocation, < 5ns latency).
    /// Returns true if written, or false if the buffer is full (overrun recorded).
    #[inline(always)]
    pub fn push(&mut self, frame: AudioFrame) -> bool {
        let write = self.write_cursor.load(Ordering::Relaxed);
        let mut read = self.cached_read_cursor.load(Ordering::Relaxed);

        // Check if cached read cursor says there's space
        if (write.wrapping_sub(read) as usize) >= RING_CAPACITY {
            // Re-fetch true atomic read cursor
            read = self.read_cursor.load(Ordering::Acquire);
            self.cached_read_cursor.store(read, Ordering::Relaxed);

            if (write.wrapping_sub(read) as usize) >= RING_CAPACITY {
                self.overrun_counter.fetch_add(1, Ordering::Relaxed);
                return false;
            }
        }

        let index = (write & RING_MASK) as usize;
        self.frames[index] = frame;

        // Release ordering guarantees frame content is globally visible before cursor advances
        self.write_cursor.store(write.wrapping_add(1), Ordering::Release);
        true
    }

    /// Read a single audio frame (Wait-Free, zero-allocation).
    /// Returns Some(AudioFrame) or None if the buffer is empty (underrun recorded).
    #[inline(always)]
    pub fn pop(&mut self) -> Option<AudioFrame> {
        let read = self.read_cursor.load(Ordering::Relaxed);
        let mut write = self.cached_write_cursor.load(Ordering::Relaxed);

        if read == write {
            // Re-fetch true atomic write cursor
            write = self.write_cursor.load(Ordering::Acquire);
            self.cached_write_cursor.store(write, Ordering::Relaxed);

            if read == write {
                self.underrun_counter.fetch_add(1, Ordering::Relaxed);
                return None;
            }
        }

        let index = (read & RING_MASK) as usize;
        let frame = self.frames[index];

        // Release ordering guarantees data is copied out before read cursor advances
        self.read_cursor.store(read.wrapping_add(1), Ordering::Release);
        Some(frame)
    }

    /// Direct zero-copy block writing for micro-buffers (e.g. 1, 8, 16, 32, 64 samples).
    /// Calls the provided closure directly with slices into the ring buffer memory.
    /// Eliminates all intermediate buffers between DMA controller and DAW.
    #[inline(always)]
    pub fn write_slice<F>(&mut self, count: usize, mut writer: F) -> usize
    where
        F: FnMut(&mut [AudioFrame]),
    {
        let write = self.write_cursor.load(Ordering::Relaxed);
        let read = self.read_cursor.load(Ordering::Acquire);
        let space = RING_CAPACITY - (write.wrapping_sub(read) as usize);

        let to_write = count.min(space);
        if to_write == 0 {
            self.overrun_counter.fetch_add(1, Ordering::Relaxed);
            return 0;
        }

        let start_idx = (write & RING_MASK) as usize;
        let end_idx = start_idx + to_write;

        if end_idx <= RING_CAPACITY {
            writer(&mut self.frames[start_idx..end_idx]);
        } else {
            let first_part = RING_CAPACITY - start_idx;
            writer(&mut self.frames[start_idx..RING_CAPACITY]);
            writer(&mut self.frames[0..(to_write - first_part)]);
        }

        self.write_cursor.store(write.wrapping_add(to_write as u64), Ordering::Release);
        to_write
    }

    /// Direct zero-copy block reading.
    #[inline(always)]
    pub fn read_slice<F>(&mut self, count: usize, mut reader: F) -> usize
    where
        F: FnMut(&[AudioFrame]),
    {
        let read = self.read_cursor.load(Ordering::Relaxed);
        let write = self.write_cursor.load(Ordering::Acquire);
        let available = (write.wrapping_sub(read) as usize).min(count);

        if available == 0 {
            self.underrun_counter.fetch_add(1, Ordering::Relaxed);
            return 0;
        }

        let start_idx = (read & RING_MASK) as usize;
        let end_idx = start_idx + available;

        if end_idx <= RING_CAPACITY {
            reader(&self.frames[start_idx..end_idx]);
        } else {
            let first_part = RING_CAPACITY - start_idx;
            reader(&self.frames[start_idx..RING_CAPACITY]);
            reader(&self.frames[0..(available - first_part)]);
        }

        self.read_cursor.store(read.wrapping_add(available as u64), Ordering::Release);
        available
    }

    /// Read raw pointers for direct Scatter-Gather hardware DMA controllers
    #[inline(always)]
    pub fn get_dma_write_pointers(&self, count: usize) -> Option<(*mut AudioFrame, usize, Option<(*mut AudioFrame, usize)>)> {
        let write = self.write_cursor.load(Ordering::Relaxed);
        let read = self.read_cursor.load(Ordering::Acquire);
        let space = RING_CAPACITY - (write.wrapping_sub(read) as usize);

        let to_write = count.min(space);
        if to_write == 0 {
            return None;
        }

        let start_idx = (write & RING_MASK) as usize;
        let base_ptr = self.frames.as_ptr() as *mut AudioFrame;

        if start_idx + to_write <= RING_CAPACITY {
            Some((unsafe { base_ptr.add(start_idx) }, to_write, None))
        } else {
            let first_len = RING_CAPACITY - start_idx;
            let second_len = to_write - first_len;
            Some((
                unsafe { base_ptr.add(start_idx) },
                first_len,
                Some((base_ptr, second_len)),
            ))
        }
    }
}
