// TLS 1.3 cryptographic primitives
//
// getrandom() — fills a buffer with cryptographically secure random bytes.

pub mod aes_gcm;
pub mod sha256;
pub mod x25519;

/// Fill `buf` with cryptographically secure random bytes.
///
/// On Linux: uses the `getrandom(2)` syscall (nr 318 on x86_64).
/// On Windows: uses `BCryptGenRandom` from bcrypt.dll.
#[cfg(all(unix, target_arch = "x86_64"))]
pub fn getrandom(buf: &mut [u8]) -> crate::Result<()> {
    use std::arch::asm;
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
    } else if (ret as usize) != buf.len() {
        Err(crate::Error::Tls("getrandom: short read".into()))
    } else {
        Ok(())
    }
}

#[cfg(all(unix, target_arch = "aarch64"))]
pub fn getrandom(buf: &mut [u8]) -> crate::Result<()> {
    use std::arch::asm;
    let ret: i64;
    unsafe {
        asm!(
            "svc #0",
            in("x8") 278u64,
            in("x0") buf.as_mut_ptr(),
            in("x1") buf.len(),
            in("x2") 0u64,
            lateout("x0") ret,
        );
    }
    if ret < 0 {
        Err(crate::Error::Tls("getrandom syscall failed".into()))
    } else if (ret as usize) != buf.len() {
        Err(crate::Error::Tls("getrandom: short read".into()))
    } else {
        Ok(())
    }
}

#[cfg(windows)]
pub fn getrandom(buf: &mut [u8]) -> crate::Result<()> {
    #[link(name = "bcrypt")]
    unsafe extern "system" {
        fn BCryptGenRandom(
            h_algorithm: *mut u8,
            pb_buffer: *mut u8,
            cb_buffer: u32,
            dw_flags: u32,
        ) -> i32;
    }
    const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x00000002;
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            buf.as_mut_ptr(),
            buf.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status != 0 {
        Err(crate::Error::Tls("BCryptGenRandom failed".into()))
    } else {
        Ok(())
    }
}
