use viv::core::net::tls::tls12::key_schedule::{
    derive_key_block, finished_verify_data, master_secret, prf,
};

#[test]
fn prf_output_length_matches_request() {
    let secret = b"test secret";
    let seed = b"test seed";
    let mut out16 = [0u8; 16];
    let mut out48 = [0u8; 48];
    prf(secret, b"label", seed, &mut out16);
    prf(secret, b"label", seed, &mut out48);
    assert_eq!(&out16[..], &out48[..16]);
}

#[test]
fn prf_is_deterministic() {
    let secret = b"secret";
    let seed = b"seed";
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    prf(secret, b"label", seed, &mut a);
    prf(secret, b"label", seed, &mut b);
    assert_eq!(a, b);
}

#[test]
fn prf_different_labels_produce_different_output() {
    let secret = b"secret";
    let seed = b"seed";
    let mut a = [0u8; 32];
    let mut b = [0u8; 32];
    prf(secret, b"label1", seed, &mut a);
    prf(secret, b"label2", seed, &mut b);
    assert_ne!(a, b);
}

#[test]
fn master_secret_is_48_bytes_deterministic() {
    let pre_master = [0x01u8; 32];
    let client_random = [0x02u8; 32];
    let server_random = [0x03u8; 32];
    let ms1 = master_secret(&pre_master, &client_random, &server_random);
    let ms2 = master_secret(&pre_master, &client_random, &server_random);
    assert_eq!(ms1.len(), 48);
    assert_eq!(ms1, ms2);
}

#[test]
fn key_block_has_correct_sizes() {
    let ms = [0u8; 48];
    let cr = [0u8; 32];
    let sr = [0u8; 32];
    let kb = derive_key_block(&ms, &sr, &cr);
    assert_eq!(kb.client_write_key.len(), 16);
    assert_eq!(kb.server_write_key.len(), 16);
    assert_eq!(kb.client_write_iv.len(), 4);
    assert_eq!(kb.server_write_iv.len(), 4);
}

#[test]
fn key_block_differs_for_different_randoms() {
    let ms = [0u8; 48];
    let cr1 = [0x01u8; 32];
    let cr2 = [0x02u8; 32];
    let sr = [0u8; 32];
    let kb1 = derive_key_block(&ms, &sr, &cr1);
    let kb2 = derive_key_block(&ms, &sr, &cr2);
    assert_ne!(kb1.client_write_key, kb2.client_write_key);
}

#[test]
fn finished_verify_data_is_12_bytes() {
    let master = [0u8; 48];
    let transcript_hash = [0u8; 32];
    let verify = finished_verify_data(&master, b"client finished", &transcript_hash);
    assert_eq!(verify.len(), 12);
}

#[test]
fn finished_verify_data_is_deterministic() {
    let master = [0x01u8; 48];
    let transcript_hash = [0x02u8; 32];
    let v1 = finished_verify_data(&master, b"client finished", &transcript_hash);
    let v2 = finished_verify_data(&master, b"client finished", &transcript_hash);
    assert_eq!(v1, v2);
}

#[test]
fn finished_verify_data_differs_for_different_labels() {
    let master = [0u8; 48];
    let transcript_hash = [0u8; 32];
    let client_v = finished_verify_data(&master, b"client finished", &transcript_hash);
    let server_v = finished_verify_data(&master, b"server finished", &transcript_hash);
    assert_ne!(client_v, server_v);
}
