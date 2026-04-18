// TLS 1.3 cryptographic primitives
//
// getrandom() — fills a buffer with random bytes via Linux getrandom syscall.

use std::arch::asm;

pub mod aes_gcm;
pub mod sha256;
pub mod x25519;

/// Fill `buf` with cryptographically secure random bytes using the
/// Linux `getrandom(2)` syscall (nr 318 on x86_64).
pub fn getrandom(buf: &mut [u8]) -> crate::Result<()> {
    let ret: i64;
    unsafe {
        asm!(
            "syscall",
            in("rax") 318u64,
            in("rdi") buf.as_mut_ptr(),
            in("rsi") buf.len(),
            in("rdx") 0u64,
            lateout("rax") ret,
            lateout("rcx") _,
            lateout("r11") _,
        );
    }
    if ret < 0 {
        Err(crate::Error::Tls("getrandom syscall failed".into()))
    } else {
        Ok(())
    }
}
