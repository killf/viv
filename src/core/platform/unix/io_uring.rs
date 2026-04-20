//! io_uring reactor for Linux.
//!
//! Replaces the epoll-based reactor for general async I/O readiness. Built
//! directly on raw `syscall()` FFI — no `libc`, no external crates. All ring
//! buffers are mmaped from the kernel and accessed via atomic loads/stores
//! with explicit memory fences, matching the protocol documented in
//! `io_uring(7)`.
//!
//! Only the subset of io_uring needed for readiness notification is wired up:
//! `POLL_ADD` (one-shot) for read/write interest, `POLL_REMOVE` for
//! deregistration, and a `TIMEOUT` op to bound `poll()` waits.

use crate::core::platform::types::RawHandle;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering, fence};
use std::task::Waker;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Raw syscall FFI
// ---------------------------------------------------------------------------

unsafe extern "C" {
    fn syscall(num: i64, ...) -> i64;
    fn __errno_location() -> *mut i32;
}

// x86_64 Linux syscall numbers
const SYS_IO_URING_SETUP: i64 = 425;
const SYS_IO_URING_ENTER: i64 = 426;
const SYS_MMAP: i64 = 9;
const SYS_MUNMAP: i64 = 11;
const SYS_CLOSE: i64 = 3;

// mmap flags
const PROT_READ: i32 = 0x1;
const PROT_WRITE: i32 = 0x2;
const MAP_SHARED: i32 = 0x01;
const MAP_POPULATE: i32 = 0x8000; // 32768

// mmap offsets for the three io_uring ring regions
const IORING_OFF_SQ_RING: i64 = 0;
const IORING_OFF_CQ_RING: i64 = 0x0800_0000;
const IORING_OFF_SQES: i64 = 0x1000_0000;

// io_uring_enter flags
const IORING_ENTER_GETEVENTS: u32 = 1 << 0;

// io_uring features
const IORING_FEAT_SINGLE_MMAP: u32 = 1 << 0;

// SQE opcodes
const IORING_OP_POLL_ADD: u8 = 6;
const IORING_OP_POLL_REMOVE: u8 = 7;
const IORING_OP_TIMEOUT: u8 = 11;

// Poll bitmask values (match kernel uapi)
const POLLIN: u32 = 0x0001;
const POLLOUT: u32 = 0x0004;
const POLLERR: u32 = 0x0008;
const POLLHUP: u32 = 0x0010;

// errno we may see from io_uring_enter when only a timeout fires
const ETIME: i32 = 62;
const EINTR: i32 = 4;

// ---------------------------------------------------------------------------
// Kernel-facing structs
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Default)]
struct IoSqringOffsets {
    head: u32,
    tail: u32,
    ring_mask: u32,
    ring_entries: u32,
    flags: u32,
    dropped: u32,
    array: u32,
    resv1: u32,
    resv2: u64,
}

#[repr(C)]
#[derive(Default)]
struct IoCqringOffsets {
    head: u32,
    tail: u32,
    ring_mask: u32,
    ring_entries: u32,
    overflow: u32,
    cqes: u32,
    flags: u32,
    resv1: u32,
    resv2: u64,
}

#[repr(C)]
#[derive(Default)]
struct IoUringParams {
    sq_entries: u32,
    cq_entries: u32,
    flags: u32,
    sq_thread_cpu: u32,
    sq_thread_idle: u32,
    features: u32,
    wq_fd: u32,
    resv: [u32; 3],
    sq_off: IoSqringOffsets,
    cq_off: IoCqringOffsets,
}

/// Submission Queue Entry — 64 bytes exact.
#[repr(C)]
#[derive(Clone, Copy)]
struct Sqe {
    opcode: u8,
    flags: u8,
    ioprio: u16,
    fd: i32,
    off: u64,
    addr: u64,
    len: u32,
    op_flags: u32, // union: poll_events / timeout_flags / rw_flags / ...
    user_data: u64,
    pad: [u64; 3], // buf_index(u16)+personality(u16)+splice_fd_in(i32)+addr3(u64)+__pad2(u64)
}

const _: () = assert!(std::mem::size_of::<Sqe>() == 64);

/// Completion Queue Entry — 16 bytes exact.
#[repr(C)]
#[derive(Clone, Copy)]
struct Cqe {
    user_data: u64,
    res: i32,
    flags: u32,
}

const _: () = assert!(std::mem::size_of::<Cqe>() == 16);

/// Kernel `struct __kernel_timespec` — 16 bytes.
#[repr(C)]
#[derive(Clone, Copy)]
struct KernelTimespec {
    tv_sec: i64,
    tv_nsec: i64,
}

// ---------------------------------------------------------------------------
// mmap helper
// ---------------------------------------------------------------------------

/// Wrapper around the mmap syscall that returns a proper `Result` instead of
/// relying on a sentinel cast. The syscall returns a signed `i64`; any
/// negative value indicates an error (errno is set by the kernel).
unsafe fn do_mmap(ring_fd: i32, offset: i64, size: usize) -> crate::Result<*mut u8> {
    let ret: i64 = unsafe {
        syscall(
            SYS_MMAP,
            0i64,
            size as i64,
            (PROT_READ | PROT_WRITE) as i64,
            (MAP_SHARED | MAP_POPULATE) as i64,
            ring_fd as i64,
            offset,
        )
    };
    if ret < 0 {
        return Err(crate::Error::Io(std::io::Error::from_raw_os_error(-ret as i32)));
    }
    Ok(ret as usize as *mut u8)
}

// ---------------------------------------------------------------------------
// IoUringReactor
// ---------------------------------------------------------------------------

pub struct IoUringReactor {
    ring_fd: i32,

    // SQ ring mmap
    sq_ptr: *mut u8,
    sq_len: usize,

    // CQ ring mmap (may alias sq_ptr when IORING_FEAT_SINGLE_MMAP is set)
    cq_ptr: *mut u8,
    cq_len: usize,
    cq_own: bool,

    // SQE array mmap
    sqes_ptr: *mut Sqe,
    sqes_len: usize,

    // SQ ring pointers (into sq_ptr)
    sq_khead: *const u32,
    sq_ktail: *mut u32,
    sq_mask: u32,
    sq_array: *mut u32,

    // CQ ring pointers (into cq_ptr)
    cq_khead: *mut u32,
    cq_ktail: *const u32,
    cq_mask: u32,
    cq_cqes: *const Cqe,

    // User-space submission tracking
    sq_tail: u32,
    pending: u32,

    // Token bookkeeping
    wakers: HashMap<u64, Waker>,
    next_token: u64,

    // Reserved tokens (must not collide with user tokens). `next_token` starts
    // above these so no user registration ever uses 0 (timeout) or 1 (probe).
    // The timeout op uses user_data=0 so it's filtered when draining the CQ.

    // Scratch timespec storage. The kernel reads this via the `addr` field of
    // the TIMEOUT SQE, so it must outlive the `io_uring_enter` call. Keeping
    // it as a struct field (rather than a stack local) guarantees a stable
    // address while we submit.
    ts: KernelTimespec,
}

// Raw pointers are owned by this reactor; shipping it between threads is
// sound as long as only one thread calls into it at a time (enforced by the
// outer `Mutex<Reactor>` wrapper).
unsafe impl Send for IoUringReactor {}

impl IoUringReactor {
    /// Create a new io_uring reactor with 32 SQEs.
    pub fn new() -> crate::Result<Self> {
        let mut params: IoUringParams = IoUringParams::default();
        let ring_fd = unsafe {
            syscall(
                SYS_IO_URING_SETUP,
                32i32,
                &mut params as *mut IoUringParams,
            )
        } as i32;
        if ring_fd < 0 {
            return Err(crate::Error::Io(std::io::Error::from_raw_os_error(unsafe {
                *__errno_location()
            })));
        }

        let sq_entries = params.sq_entries;
        let cq_entries = params.cq_entries;

        // ---- SQ ring mmap ----
        let sq_len = (params.sq_off.array + sq_entries * 4) as usize;
        let sq_ptr = match unsafe { do_mmap(ring_fd, IORING_OFF_SQ_RING, sq_len) } {
            Ok(p) => p,
            Err(e) => {
                unsafe { syscall(SYS_CLOSE, ring_fd) };
                return Err(e);
            }
        };

        // ---- CQ ring mmap (maybe shared with SQ) ----
        let cq_own = (params.features & IORING_FEAT_SINGLE_MMAP) == 0;
        let cq_len_calc = (params.cq_off.cqes + cq_entries * 16) as usize;
        let (cq_ptr, cq_len) = if !cq_own {
            (sq_ptr, cq_len_calc)
        } else {
            match unsafe { do_mmap(ring_fd, IORING_OFF_CQ_RING, cq_len_calc) } {
                Ok(p) => (p, cq_len_calc),
                Err(e) => {
                    unsafe { syscall(SYS_MUNMAP, sq_ptr as usize, sq_len) };
                    unsafe { syscall(SYS_CLOSE, ring_fd) };
                    return Err(e);
                }
            }
        };

        // ---- SQEs mmap ----
        let sqes_len = (sq_entries as usize) * 64;
        let sqes_ptr = match unsafe { do_mmap(ring_fd, IORING_OFF_SQES, sqes_len) } {
            Ok(p) => p as *mut Sqe,
            Err(e) => {
                if cq_own {
                    unsafe { syscall(SYS_MUNMAP, cq_ptr as usize, cq_len) };
                }
                unsafe { syscall(SYS_MUNMAP, sq_ptr as usize, sq_len) };
                unsafe { syscall(SYS_CLOSE, ring_fd) };
                return Err(e);
            }
        };

        // ---- Resolve ring field pointers from the byte offsets ----
        let sq_khead = unsafe { sq_ptr.add(params.sq_off.head as usize) } as *const u32;
        let sq_ktail = unsafe { sq_ptr.add(params.sq_off.tail as usize) } as *mut u32;
        let sq_mask =
            unsafe { *(sq_ptr.add(params.sq_off.ring_mask as usize) as *const u32) };
        let sq_array = unsafe { sq_ptr.add(params.sq_off.array as usize) } as *mut u32;

        let cq_khead = unsafe { cq_ptr.add(params.cq_off.head as usize) } as *mut u32;
        let cq_ktail = unsafe { cq_ptr.add(params.cq_off.tail as usize) } as *const u32;
        let cq_mask =
            unsafe { *(cq_ptr.add(params.cq_off.ring_mask as usize) as *const u32) };
        let cq_cqes = unsafe { cq_ptr.add(params.cq_off.cqes as usize) } as *const Cqe;

        let sq_tail_init = unsafe { *(sq_ktail as *const u32) };

        Ok(IoUringReactor {
            ring_fd,
            sq_ptr,
            sq_len,
            cq_ptr,
            cq_len,
            cq_own,
            sqes_ptr,
            sqes_len,
            sq_khead,
            sq_ktail,
            sq_mask,
            sq_array,
            cq_khead,
            cq_ktail,
            cq_mask,
            cq_cqes,
            sq_tail: sq_tail_init,
            pending: 0,
            wakers: HashMap::new(),
            // Leave room for reserved tokens (0 = timeout). Start at 2 for a
            // little safety margin.
            next_token: 2,
            ts: KernelTimespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
        })
    }

    fn sq_entries(&self) -> u32 {
        self.sq_mask + 1
    }

    /// Fill SQE slot indexed by current `sq_tail` with the provided op and
    /// advance `sq_tail`. Returns `Err` on full-ring.
    fn submit_sqe(
        &mut self,
        opcode: u8,
        fd: i32,
        addr: u64,
        len: u32,
        op_flags: u32,
        off: u64,
        user_data: u64,
    ) -> crate::Result<()> {
        // Load kernel's head with Acquire so we observe any previously
        // consumed SQEs.
        let khead = unsafe { (*(self.sq_khead as *const AtomicU32)).load(Ordering::Acquire) };
        if self.sq_tail.wrapping_sub(khead) >= self.sq_entries() {
            return Err(crate::Error::Io(std::io::Error::other(
                "io_uring submission queue full",
            )));
        }
        let idx = (self.sq_tail & self.sq_mask) as usize;

        let sqe = Sqe {
            opcode,
            flags: 0,
            ioprio: 0,
            fd,
            off,
            addr,
            len,
            op_flags,
            user_data,
            pad: [0; 3],
        };
        unsafe {
            self.sqes_ptr.add(idx).write(sqe);
            // Identity mapping of sq_array[idx] = idx — standard for
            // non-SQPOLL rings.
            self.sq_array.add(idx).write(idx as u32);
        }
        self.sq_tail = self.sq_tail.wrapping_add(1);
        self.pending += 1;
        Ok(())
    }

    /// Register readable interest on `handle`. Uses one-shot POLL_ADD.
    pub fn register_read(&mut self, handle: RawHandle, waker: Waker) -> crate::Result<u64> {
        let token = self.next_token;
        self.next_token = self.next_token.wrapping_add(1);
        let events = POLLIN | POLLHUP | POLLERR;
        self.submit_sqe(IORING_OP_POLL_ADD, handle, 0, 0, events, 0, token)?;
        self.wakers.insert(token, waker);
        Ok(token)
    }

    /// Register writable interest on `handle`. Uses one-shot POLL_ADD.
    pub fn register_write(&mut self, handle: RawHandle, waker: Waker) -> crate::Result<u64> {
        let token = self.next_token;
        self.next_token = self.next_token.wrapping_add(1);
        let events = POLLOUT | POLLERR;
        self.submit_sqe(IORING_OP_POLL_ADD, handle, 0, 0, events, 0, token)?;
        self.wakers.insert(token, waker);
        Ok(token)
    }

    /// Cancel a pending registration. Best-effort: if the CQE for `token`
    /// already arrived (waker already fired), the POLL_REMOVE will get a
    /// no-op completion, which we silently drop.
    pub fn deregister(&mut self, token: u64) -> crate::Result<()> {
        if self.wakers.remove(&token).is_some() {
            // Submit a POLL_REMOVE; even if it races with a completion that's
            // fine — both paths converge to "no waker, drop the CQE".
            // user_data=1 so we can distinguish it from user tokens if needed
            // (we don't — any user_data without a registered waker is ignored
            // on drain).
            let _ = self.submit_sqe(IORING_OP_POLL_REMOVE, -1, token, 0, 0, 0, 1);
        }
        Ok(())
    }

    /// Flush pending SQEs and wait up to `timeout` for at least one
    /// completion, then drain the CQ and wake matching wakers. Returns the
    /// number of wakers fired.
    pub fn poll(&mut self, timeout: Duration) -> crate::Result<usize> {
        // Submit a TIMEOUT op so the kernel will return after `timeout` even
        // if no I/O completes. user_data=0 is our reserved sentinel — the CQE
        // it produces has no waker attached and gets filtered on drain.
        self.ts = KernelTimespec {
            tv_sec: timeout.as_secs() as i64,
            tv_nsec: timeout.subsec_nanos() as i64,
        };
        let ts_addr = &self.ts as *const KernelTimespec as u64;
        // Best-effort submit of the timeout; if the SQ is full we still call
        // io_uring_enter below — the enter call will consume submissions and
        // wait for at least one completion, and the next poll() will retry
        // the timeout.
        let _ = self.submit_sqe(IORING_OP_TIMEOUT, -1, ts_addr, 1, 0, 0, 0);

        // Publish sq_tail to the kernel with a Release fence so the SQE
        // writes are visible first.
        fence(Ordering::Release);
        unsafe {
            (*(self.sq_ktail as *const AtomicU32)).store(self.sq_tail, Ordering::Relaxed);
        }

        let to_submit = self.pending;
        // Wait for at least one completion. The TIMEOUT CQE counts, so we
        // will always return within `timeout`.
        let ret = unsafe {
            syscall(
                SYS_IO_URING_ENTER,
                self.ring_fd,
                to_submit,
                1u32,
                IORING_ENTER_GETEVENTS,
                0usize,
                0usize,
            )
        };
        if ret < 0 {
            let errno = unsafe { *__errno_location() };
            if errno == EINTR {
                // Signal interrupted before any submissions were consumed.
                // Do not reset `pending` — re-submit on next poll().
                return Ok(0);
            } else if errno != ETIME {
                return Err(crate::Error::Io(std::io::Error::from_raw_os_error(errno)));
            }
            // ETIME: timeout fired; kernel accepted all submissions.
        }
        // Success or ETIME: everything in the SQ has been submitted.
        self.pending = 0;

        // ---- Drain completions ----
        let mut head = unsafe { (*(self.cq_khead as *const AtomicU32)).load(Ordering::Relaxed) };
        fence(Ordering::Acquire);
        let tail = unsafe { (*(self.cq_ktail as *const AtomicU32)).load(Ordering::Relaxed) };

        let mut woke: usize = 0;
        while head != tail {
            let idx = (head & self.cq_mask) as usize;
            let cqe = unsafe { *self.cq_cqes.add(idx) };
            // user_data == 0 is our timeout sentinel; ignore.
            if cqe.user_data != 0 {
                if let Some(waker) = self.wakers.remove(&cqe.user_data) {
                    waker.wake();
                    woke += 1;
                }
                // Anything else (e.g. POLL_REMOVE completions with
                // user_data=1, or stale completions after a dropped waker) is
                // silently discarded.
            }
            head = head.wrapping_add(1);
        }
        fence(Ordering::Release);
        unsafe {
            (*(self.cq_khead as *const AtomicU32)).store(head, Ordering::Relaxed);
        }

        Ok(woke)
    }
}

impl Drop for IoUringReactor {
    fn drop(&mut self) {
        unsafe {
            if !self.sq_ptr.is_null() {
                syscall(SYS_MUNMAP, self.sq_ptr as usize, self.sq_len);
            }
            if self.cq_own && !self.cq_ptr.is_null() {
                syscall(SYS_MUNMAP, self.cq_ptr as usize, self.cq_len);
            }
            if !self.sqes_ptr.is_null() {
                syscall(SYS_MUNMAP, self.sqes_ptr as usize, self.sqes_len);
            }
            if self.ring_fd >= 0 {
                syscall(SYS_CLOSE, self.ring_fd);
            }
        }
    }
}
