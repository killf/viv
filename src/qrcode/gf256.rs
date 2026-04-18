/// GF(256) finite field arithmetic with primitive polynomial x^8 + x^4 + x^3 + x^2 + 1 (0x11D).

const fn build_tables() -> ([u8; 256], [u8; 256]) {
    let mut exp = [0u8; 256];
    let mut log = [0u8; 256];

    let mut x: u32 = 1;
    let mut i = 0usize;
    while i < 255 {
        exp[i] = x as u8;
        log[x as usize] = i as u8;
        x <<= 1;
        if x >= 256 {
            x ^= 0x11D;
        }
        i += 1;
    }
    // EXP_TABLE[255] wraps to same as EXP_TABLE[0] for convenience
    exp[255] = exp[0];

    (exp, log)
}

const TABLES: ([u8; 256], [u8; 256]) = build_tables();

/// EXP_TABLE[i] = α^i in GF(256).
pub static EXP_TABLE: [u8; 256] = TABLES.0;

/// LOG_TABLE[v] = i such that α^i = v. LOG_TABLE[0] is undefined (set to 0).
pub static LOG_TABLE: [u8; 256] = TABLES.1;

/// Multiply two elements in GF(256).
pub fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    EXP_TABLE[(LOG_TABLE[a as usize] as usize + LOG_TABLE[b as usize] as usize) % 255]
}

/// Divide two elements in GF(256). Panics if b == 0.
pub fn div(a: u8, b: u8) -> u8 {
    if a == 0 {
        return 0;
    }
    EXP_TABLE[(LOG_TABLE[a as usize] as usize + 255 - LOG_TABLE[b as usize] as usize) % 255]
}

/// Raise an element to a power in GF(256).
pub fn pow(a: u8, n: u32) -> u8 {
    if n == 0 {
        return 1;
    }
    if a == 0 {
        return 0;
    }
    EXP_TABLE[(LOG_TABLE[a as usize] as u32 * n % 255) as usize]
}
