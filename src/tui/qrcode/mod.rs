pub mod encode;
pub mod gf256;
pub mod matrix;
pub mod rs;
pub mod tables;

pub use matrix::QrMatrix;

/// Encode UTF-8 text into a QR code matrix.
pub fn encode(text: &str) -> crate::Result<QrMatrix> {
    let encoded = encode::encode_and_interleave(text)?;
    Ok(QrMatrix::build(encoded.version, &encoded.data))
}
