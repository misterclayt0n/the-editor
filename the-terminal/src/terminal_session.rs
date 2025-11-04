//! Complete terminal session combining VT emulation and PTY management
//!
//! This module provides `TerminalSession` which wraps both the Terminal (VT
//! emulation) and PtySession (process management) into a single cohesive
//! interface.
//!
//! Architecture: Terminal state is wrapped in Arc<Mutex<>> to allow safe access
//! from the dedicated PTY read thread and the main/render thread.

use std::{
  ffi::c_void,
  sync::{
    Arc,
    Mutex,
    atomic::{
      AtomicBool,
      AtomicU64,
      Ordering,
    },
  },
  time::{
    Duration,
    Instant,
  },
};

use anyhow::Result;
use crossbeam::queue::ArrayQueue;

type RedrawCallback = Arc<dyn Fn() + Send + Sync + 'static>;

use crate::{
  Terminal,
  pty::{
    OutputCallback,
    PtySession,
  },
};

/// A complete terminal session: VT100 emulation + PTY process
///
/// This combines:
/// - `Terminal`: Parses VT100 escape sequences and maintains terminal state
/// - `PtySession`: Manages the shell process and I/O
///
/// Architecture:
/// - Terminal state is in Arc<Mutex<>> for thread-safe access
/// - PTY read thread writes directly to terminal via callback
/// - Main thread locks terminal for rendering and sending responses
/// - Redraw flag signals when terminal needs re-rendering
///
/// Usage pattern:
/// ```no_run
/// # use the_terminal::TerminalSession;
/// # async fn example() -> anyhow::Result<()> {
/// let mut session = TerminalSession::new(24, 80, None)?;
///
/// // In your render loop:
/// loop {
///   // Check if terminal needs redraw
///   if session.needs_redraw() {
///     // Lock and render terminal state
///     let terminal = session.lock_terminal();
///     let grid = terminal.grid();
///     // ... render grid to UI
///     drop(terminal); // Release lock
///     session.clear_redraw_flag();
///   }
///
///   // Handle user input
///   session.send_input(b"echo hello\n".to_vec())?;
///
///   if !session.is_alive() {
///     break; // Shell exited
///   }
/// }
/// # Ok(())
/// # }
/// ```
pub struct TerminalSession {
  /// VT100 terminal emulator (thread-safe)
  /// Accessed by PTY read thread (via callback) and main thread (for rendering)
  terminal: Arc<Mutex<Terminal>>,

  /// PTY session with shell process
  pty: PtySession,

  /// Lock-free queue for terminal responses (e.g., cursor position reports)
  /// Bounded to prevent unbounded memory growth. Callbacks push here,
  /// process_responses() drains and sends to PTY.
  response_queue: Arc<ArrayQueue<Vec<u8>>>,

  /// Raw pointer to callback context (Arc<ArrayQueue>)
  /// Must be cleaned up in Drop by reconstructing the Arc
  callback_ctx_ptr: *mut c_void,

  /// Flag indicating terminal needs redraw
  /// Set by PTY read thread when data arrives, cleared by render thread
  needs_redraw: Arc<AtomicBool>,

  /// Flag indicating terminal needs a full render (all rows)
  /// Set when terminal is reattached, resized, or state becomes stale
  /// Cleared after performing full render
  needs_full_render: Arc<AtomicBool>,

  /// Last cell size in pixels (width, height)
  cell_pixel_size: (u16, u16),

  /// Background color reported for OSC queries
  background_color: (u8, u8, u8),
  /// Foreground color reported for OSC queries
  foreground_color: (u8, u8, u8),

  /// Last render time for FPS throttling
  /// Prevents excessive redraws when PTY outputs fast
  last_render_time: Instant,

  /// Maximum FPS for terminal rendering (default: 120)
  /// Can be overridden via configuration
  max_fps: u32,

  /// Optional callback invoked whenever the terminal requests a redraw.
  redraw_notifier: Arc<Mutex<Option<RedrawCallback>>>,

  /// Last time we notified about a redraw (nanoseconds since arbitrary epoch)
  /// Used to throttle redraw notifications and prevent event system saturation
  last_notify_time: Arc<AtomicU64>,
}

impl TerminalSession {
  /// Create a new terminal session with a shell process
  ///
  /// # Arguments
  /// * `rows` - Number of terminal rows
  /// * `cols` - Number of terminal columns
  /// * `shell` - Optional shell command. If None, uses $SHELL or /bin/bash
  pub fn new(rows: u16, cols: u16, shell: Option<&str>) -> Result<Self> {
    let mut terminal = Terminal::new(cols, rows)?;

    // Create bounded queue for terminal responses (capacity 64, matching ghostty)
    // This is lock-free and can be safely called from any thread
    let response_queue = Arc::new(ArrayQueue::new(64));

    // CRITICAL: Clone the Arc BEFORE converting to raw pointer.
    // Arc::into_raw consumes the Arc without incrementing refcount.
    // We need TWO strong references:
    //   1. response_queue field (keeps data alive)
    //   2. callback_ctx_ptr (for FFI callback access)
    let queue_for_callback = Arc::clone(&response_queue);
    let queue_ptr = Arc::into_raw(queue_for_callback) as *mut c_void;

    // SAFETY: The queue pointer will remain valid because:
    // - response_queue field holds one strong Arc reference
    // - callback_ctx_ptr holds another strong reference (as raw pointer)
    // - In Drop, we reconstruct the Arc from raw to properly decrement refcount
    unsafe {
      terminal.set_response_callback_raw(Self::response_callback, queue_ptr);
    }

    // Wrap terminal in Arc<Mutex<>> for thread-safe access
    let terminal = Arc::new(Mutex::new(terminal));

    // Create redraw flag
    let needs_redraw = Arc::new(AtomicBool::new(true));

    // Create full render flag (initially true for first render)
    let needs_full_render = Arc::new(AtomicBool::new(true));

    // Create output callback that writes to terminal
    // This will be called from the PTY read thread
    let terminal_for_callback = Arc::clone(&terminal);
    let needs_redraw_for_callback = Arc::clone(&needs_redraw);
    let redraw_notifier = Arc::new(Mutex::new(None::<RedrawCallback>));
    let redraw_notifier_for_callback = Arc::clone(&redraw_notifier);
    let last_notify_time = Arc::new(AtomicU64::new(0));
    let last_notify_time_for_callback = Arc::clone(&last_notify_time);

    let output_callback: OutputCallback = Arc::new(move |data: &[u8]| {
      // Lock terminal and write data
      if let Ok(mut term) = terminal_for_callback.lock() {
        if let Err(e) = term.write(data) {
          log::error!("Terminal write error: {}", e);
        }
        // Set redraw flag
        needs_redraw_for_callback.store(true, Ordering::Release);
      } else {
        log::error!("Failed to lock terminal for write");
      }

      // Throttle redraw notifications to prevent event system saturation
      // This is the critical fix for performance with large outputs (ps, etc.)
      // Similar to Ghostty's 25ms coalescing window, we use 16ms (~60 FPS)
      const MIN_NOTIFY_INTERVAL_NS: u64 = 16_000_000; // 16ms

      let now = Instant::now().elapsed().as_nanos() as u64;
      let last = last_notify_time_for_callback.load(Ordering::Relaxed);

      // Only notify if enough time has passed since last notification
      if now.saturating_sub(last) >= MIN_NOTIFY_INTERVAL_NS {
        // Try to update the timestamp atomically
        // If another thread beat us to it, that's fine - they'll notify
        if last_notify_time_for_callback
          .compare_exchange(last, now, Ordering::Release, Ordering::Relaxed)
          .is_ok()
        {
          // We won the race - send the notification
          let callback = redraw_notifier_for_callback
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(Arc::clone));
          if let Some(cb) = callback {
            cb();
          }
        }
      }
    });

    // Create PTY with output callback
    let pty = PtySession::new(rows, cols, shell, output_callback)?;

    Ok(Self {
      terminal,
      pty,
      response_queue,
      callback_ctx_ptr: queue_ptr,
      needs_redraw,
      needs_full_render,
      cell_pixel_size: (0, 0),
      background_color: (0, 0, 0),
      foreground_color: (255, 255, 255),
      last_render_time: Instant::now(),
      max_fps: 120,
      redraw_notifier,
      last_notify_time,
    })
  }

  /// Callback invoked by the terminal when it needs to send responses to the
  /// PTY
  ///
  /// # Safety
  /// This is called from Zig FFI and must be thread-safe.
  /// The ctx pointer is an Arc<ArrayQueue> from Arc::into_raw.
  /// ArrayQueue is lock-free so this is safe to call from any thread.
  extern "C" fn response_callback(ctx: *mut c_void, data: *const u8, len: usize) {
    // SAFETY: ctx is a pointer to ArrayQueue<Vec<u8>> obtained via Arc::into_raw in
    // new(). We reborrow the queue without adjusting the strong count;
    // ownership stays with the Arc in TerminalSession.
    let queue = unsafe { &*(ctx as *const ArrayQueue<Vec<u8>>) };

    // Convert the slice to a Vec
    let response = unsafe { std::slice::from_raw_parts(data, len).to_vec() };

    // Debug: log what we're sending
    log::debug!(
      "Terminal response ({} bytes): {:?}",
      len,
      String::from_utf8_lossy(&response)
    );

    // Push to queue, drop if full (backpressure)
    // This is lock-free and safe from any thread
    if queue.push(response).is_err() {
      log::warn!("Terminal response queue full, dropping message");
    }
  }

  /// Process pending terminal responses and send them to PTY
  ///
  /// Terminal responses (e.g., cursor position reports, OSC queries) are queued
  /// by the terminal emulator and need to be sent back to the shell.
  ///
  /// This should be called periodically, but is less critical than in the old
  /// architecture since PTY output is now processed in a dedicated thread.
  pub fn process_responses(&mut self) {
    // Drain response queue and send to PTY
    // Rate limit to prevent infinite loops (max 16 responses per call)
    const MAX_RESPONSES_PER_CALL: usize = 16;

    for _ in 0..MAX_RESPONSES_PER_CALL {
      match self.response_queue.pop() {
        Some(response) => {
          // Send response to PTY input (shell's stdin)
          if let Err(e) = self.pty.send_input(response) {
            log::warn!("Failed to send terminal response to PTY: {}", e);
            break; // Stop if PTY is closed
          }
        },
        None => break, // Queue empty
      }
    }
  }

  /// Check if terminal needs redraw
  ///
  /// Returns true if PTY output has been processed since last
  /// clear_redraw_flag()
  pub fn needs_redraw(&self) -> bool {
    self.needs_redraw.load(Ordering::Acquire)
  }

  /// Clear the redraw flag
  ///
  /// Should be called after rendering the terminal
  pub fn clear_redraw_flag(&self) {
    self.needs_redraw.store(false, Ordering::Release);
  }

  /// Check if terminal needs a full render (all rows)
  ///
  /// Returns true if terminal was resized, reattached, or needs state reset
  pub fn needs_full_render(&self) -> bool {
    self.needs_full_render.load(Ordering::Acquire)
  }

  /// Mark terminal as needing a redraw.
  ///
  /// Useful when the UI state changes (e.g. pane reattachment) and we must
  /// schedule a frame even if no new PTY output arrived yet.
  pub fn mark_needs_redraw(&self) {
    self.needs_redraw.store(true, Ordering::Release);
    self.notify_redraw_listeners();
  }

  /// Mark terminal as needing a full render
  ///
  /// Call this when terminal is reattached after being detached,
  /// or when dirty state may be stale/inconsistent
  pub fn mark_needs_full_render(&self) {
    self.needs_full_render.store(true, Ordering::Release);
    self.mark_needs_redraw();
  }

  /// Clear the full render flag
  ///
  /// Should be called after performing a full render
  pub fn clear_full_render_flag(&self) {
    self.needs_full_render.store(false, Ordering::Release);
  }

  /// Register or clear a redraw notifier callback.
  pub fn set_redraw_notifier(&self, notifier: Option<RedrawCallback>) {
    if let Ok(mut guard) = self.redraw_notifier.lock() {
      *guard = notifier;
    }
  }

  fn notify_redraw_listeners(&self) {
    let callback = self
      .redraw_notifier
      .lock()
      .ok()
      .and_then(|guard| guard.as_ref().map(Arc::clone));
    if let Some(cb) = callback {
      cb();
    }
  }

  /// Lock the terminal for reading/rendering
  ///
  /// Returns a mutex guard that provides access to the terminal state.
  /// The lock is released when the guard is dropped.
  pub fn lock_terminal(&self) -> std::sync::MutexGuard<'_, Terminal> {
    self.terminal.lock().unwrap()
  }

  /// Get the list of dirty rows that need re-rendering
  ///
  /// This is a convenience method that locks the terminal and returns the
  /// list of dirty row indices.
  pub fn get_dirty_rows(&self) -> Vec<u32> {
    if let Ok(term) = self.terminal.lock() {
      term.get_dirty_rows()
    } else {
      Vec::new()
    }
  }

  /// Atomically get dirty rows and clear them in one operation
  ///
  /// This is the preferred method for rendering as it prevents race conditions
  /// between reading dirty state and clearing it. The PTY thread cannot set new
  /// dirty bits between these operations.
  ///
  /// Returns an empty Vec when a full rebuild is needed (either from Ghostty's
  /// terminal-level dirty flags or our own needs_full_render flag). The caller
  /// should render all rows in this case.
  ///
  /// CRITICAL FIX: This now checks Ghostty's terminal-level dirty flags (set by
  /// eraseDisplay, resize, etc.) BEFORE checking row-level dirty bits. This is
  /// the root cause fix for nushell performance - nushell's prompt redraws
  /// trigger eraseDisplay which sets terminal.flags.dirty.clear, not
  /// row-level bits.
  ///
  /// CRITICAL: This prevents the race condition where:
  /// 1. Render thread reads dirty rows
  /// 2. PTY thread sets new dirty bits
  /// 3. Render thread clears ALL dirty bits (including new ones from step 2)
  /// 4. Next frame misses the updates from step 2
  pub fn get_and_clear_dirty_rows(&self) -> Vec<u32> {
    if let Ok(mut term) = self.terminal.lock() {
      // CRITICAL: Check Ghostty's terminal-level dirty flags FIRST
      // These are set by operations like eraseDisplay, resize, mode changes
      // If set, we need to render all rows, not just row-level dirty bits
      let needs_ghostty_rebuild = term.needs_full_rebuild();
      let needs_our_rebuild = self.needs_full_render();

      if needs_ghostty_rebuild || needs_our_rebuild {
        // Full render needed - clear ALL dirty state and return empty vec
        term.clear_dirty(); // Clears terminal-level AND row-level dirty bits
        if needs_our_rebuild {
          self.clear_full_render_flag();
        }
        return Vec::new(); // Signals full render to caller
      }

      // Incremental render - get and clear only row-level dirty bits
      let dirty_rows = term.get_dirty_rows();
      term.clear_dirty();
      dirty_rows
    } else {
      Vec::new()
    }
  }

  /// Clear the dirty bits after rendering
  ///
  /// This is a convenience method that locks the terminal and clears all
  /// dirty bits. Should be called after rendering all dirty rows.
  pub fn clear_dirty_bits(&self) {
    if let Ok(mut term) = self.terminal.lock() {
      term.clear_dirty();
    }
  }

  /// Send input to the PTY (keyboard input from user)
  ///
  /// # Arguments
  /// * `data` - Bytes to send to shell (e.g., keyboard input or VT100
  ///   sequences)
  ///
  /// # Example
  /// ```no_run
  /// # use the_terminal::TerminalSession;
  /// # async fn example() -> anyhow::Result<()> {
  /// # let mut session = TerminalSession::new(24, 80, None)?;
  /// // Send "echo hello"
  /// session.send_input(b"echo hello\n".to_vec())?;
  /// # Ok(())
  /// # }
  /// ```
  pub fn send_input(&self, data: Vec<u8>) -> Result<()> {
    self.pty.send_input(data)
  }

  /// Resize the terminal and PTY
  ///
  /// This updates both the Terminal emulation size and the PTY size,
  /// sending SIGWINCH to the shell process.
  pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
    // Resize Terminal emulation (lock required)
    if let Ok(mut term) = self.terminal.lock() {
      term.resize(cols, rows)?;
    }

    // Resize PTY (sends SIGWINCH to shell)
    self.pty.resize(rows, cols)?;

    Ok(())
  }

  /// Get current terminal dimensions
  pub fn size(&self) -> (u16, u16) {
    self.pty.size()
  }

  /// Check if the shell process is still alive
  ///
  /// Returns false when the shell has exited.
  pub fn is_alive(&self) -> bool {
    self.pty.is_alive()
  }

  /// Kill the shell process
  pub async fn kill(&mut self) -> Result<()> {
    self.pty.kill().await
  }

  /// Update cached cell metrics (in pixels) used for reporting queries.
  pub fn set_cell_pixel_size(&mut self, width: f32, height: f32) {
    let clamp = |value: f32| -> u16 {
      let rounded = value.round().max(1.0);
      rounded.min(u16::MAX as f32) as u16
    };

    let width_px = clamp(width);
    let height_px = clamp(height);
    self.cell_pixel_size = (width_px, height_px);

    // Update terminal (lock required)
    if let Ok(mut term) = self.terminal.lock() {
      term.set_cell_pixel_size(width_px, height_px);
    }
  }

  /// Update the background color used to answer OSC color queries.
  pub fn set_background_color(&mut self, r: u8, g: u8, b: u8) {
    self.background_color = (r, g, b);

    // Update terminal (lock required)
    if let Ok(mut term) = self.terminal.lock() {
      term.set_background_color(r, g, b);
    }
  }

  /// Update the foreground color used to answer OSC color queries.
  pub fn set_foreground_color(&mut self, r: u8, g: u8, b: u8) {
    self.foreground_color = (r, g, b);

    if let Ok(mut term) = self.terminal.lock() {
      term.set_foreground_color(r, g, b);
    }
  }

  /// Override a palette entry.
  pub fn set_palette_color(&mut self, index: u16, r: u8, g: u8, b: u8) {
    if let Ok(mut term) = self.terminal.lock() {
      if term.set_palette_color(index, r, g, b) {
        self.needs_redraw.store(true, Ordering::Release);
      }
    }
  }

  /// Create a snapshot of the terminal screen for rendering.
  ///
  /// This implements ghostty's clone-and-release pattern for minimal lock
  /// contention. Captures cursor position, size, and dirty row indices
  /// atomically, then clears dirty bits before releasing the lock. Cell data
  /// is accessed later via pins.
  ///
  /// **Lock hold time**: Typically 10-100 microseconds
  /// - Metadata copy: ~1-10 µs (just integers)
  /// - Dirty row scan: ~1-50 µs (iterate row dirty bits)
  /// - Dirty row allocation: ~1-20 µs (Vec allocation)
  /// - Clear dirty: ~1-10 µs (bitset clear)
  ///
  /// **NOTE**: Dirty row allocation (Vec) happens while holding lock.
  /// This is not ideal but acceptable. Future optimization: use row iterator
  /// during rendering instead of extracting dirty rows to Vec (see tasks 4-6).
  ///
  /// **Correctness**: Atomically snapshots which rows are dirty before
  /// clearing, ensuring no updates are missed between snapshot and clear
  /// operations.
  ///
  /// Returns None if terminal lock is poisoned.
  pub fn create_screen_snapshot(&self) -> Option<crate::terminal::ScreenSnapshot> {
    if let Ok(mut term) = self.terminal.lock() {
      // Create snapshot (ONLY metadata - cursor, size, dirty rows)
      // NOTE: This allocates Vec for dirty rows while holding lock.
      // Future: use row iterator pattern (ghostty's approach) instead.
      let snapshot = crate::terminal::ScreenSnapshot::from_terminal(&term);

      // Atomically clear dirty bits after snapshotting
      // This prevents race conditions with PTY thread
      term.clear_dirty();

      Some(snapshot)
    } else {
      None
    }
  }

  /// Check if enough time has passed to render at configured FPS.
  ///
  /// This throttles terminal rendering to prevent excessive CPU usage
  /// when the PTY produces output faster than the display can refresh.
  /// Implements frame rate limiting similar to Ghostty (default: 120 FPS).
  ///
  /// Returns true if rendering should proceed, false if it should be deferred.
  pub fn can_render(&mut self) -> bool {
    let now = Instant::now();
    let min_frame_time = Duration::from_millis(1000 / self.max_fps as u64);

    if now.duration_since(self.last_render_time) >= min_frame_time {
      self.last_render_time = now;
      true
    } else {
      false
    }
  }

  /// Set the maximum FPS for terminal rendering.
  pub fn set_max_fps(&mut self, fps: u32) {
    self.max_fps = fps.max(1).min(1000); // Clamp to 1-1000 FPS
  }

  /// Get the current maximum FPS setting.
  pub fn max_fps(&self) -> u32 {
    self.max_fps
  }

  /// Reset the throttle timer (used after successful renders).
  pub fn reset_render_timer(&mut self) {
    self.last_render_time = Instant::now();
  }
}

impl Drop for TerminalSession {
  fn drop(&mut self) {
    // Clean up the callback context pointer by reconstructing the Arc
    // This properly decrements the refcount and frees memory if needed
    if !self.callback_ctx_ptr.is_null() {
      log::debug!("Cleaning up terminal callback context");

      // SAFETY: This pointer was created with Arc::into_raw in new()
      // Reconstructing the Arc will properly decrement the refcount
      unsafe {
        let _arc = Arc::from_raw(self.callback_ctx_ptr as *const ArrayQueue<Vec<u8>>);
        // Arc is dropped here, decrementing refcount
      }

      // Prevent double-free
      self.callback_ctx_ptr = std::ptr::null_mut();
    }
  }
}

// Tests are in ../tests/pty_integration.rs to handle shell discovery
