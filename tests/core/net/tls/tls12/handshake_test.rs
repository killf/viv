use viv::core::net::tls::tls12::handshake::{Tls12Handshake, Tls12HandshakeResult};
use viv::core::net::tls::tls12::record::Tls12RecordLayer;
use viv::core::net::tls::p256::Point;

fn make_handshake() -> Tls12Handshake {
    Tls12Handshake::new(&[0x01u8; 32], &[0x02u8; 32], 0xC02B).unwrap()
}

fn make_cert_msg() -> Vec<u8> {
    // Certificate body parsed by decode_certificate:
    //   ctx_len(1=0) + list_len(3=0) = 4 bytes, empty cert list
    let body = vec![0x00, 0x00, 0x00, 0x00];
    let mut msg = vec![0x0B]; // CERTIFICATE
    let len = body.len() as u32;
    msg.push((len >> 16) as u8);
    msg.push((len >> 8) as u8);
    msg.push(len as u8);
    msg.extend_from_slice(&body);
    msg
}

fn make_ske_msg() -> Vec<u8> {
    // ServerKeyExchange with valid P-256 public key (generator * 3)
    let server_secret = [3u8; 32];
    let server_pub = Point::generator().scalar_mul(&server_secret)
        .to_uncompressed().unwrap();
    let fake_sig = [0u8; 64];
    let mut body = Vec::new();
    body.push(3); // curve_type = named_curve
    body.extend_from_slice(&[0x00, 0x17]); // secp256r1
    body.push(65); // pubkey length
    body.extend_from_slice(&server_pub);
    body.extend_from_slice(&[0x04, 0x01]); // sig_alg
    body.extend_from_slice(&[0x00, 64u8]); // sig length
    body.extend_from_slice(&fake_sig);
    let mut msg = vec![0x0C]; // SERVER_KEY_EXCHANGE
    let len = body.len() as u32;
    msg.push((len >> 16) as u8);
    msg.push((len >> 8) as u8);
    msg.push(len as u8);
    msg.extend_from_slice(&body);
    msg
}

fn make_shd_msg() -> Vec<u8> {
    vec![0x0E, 0x00, 0x00, 0x00] // SERVER_HELLO_DONE, len=0
}

#[test]
fn certificate_advances_state() {
    let mut hs = make_handshake();
    let mut record = Tls12RecordLayer::new();
    let result = hs.handle_message(&make_cert_msg(), &mut record).unwrap();
    assert!(matches!(result, Tls12HandshakeResult::Continue));
}

#[test]
fn server_hello_done_returns_send_to_server() {
    let mut hs = make_handshake();
    let mut record = Tls12RecordLayer::new();

    hs.handle_message(&make_cert_msg(), &mut record).unwrap();
    hs.handle_message(&make_ske_msg(), &mut record).unwrap();
    let result = hs.handle_message(&make_shd_msg(), &mut record).unwrap();

    assert!(
        matches!(result, Tls12HandshakeResult::SendToServer(_)),
        "Expected SendToServer after ServerHelloDone"
    );
}

#[test]
fn send_to_server_contains_ccs_byte() {
    let mut hs = make_handshake();
    let mut record = Tls12RecordLayer::new();

    hs.handle_message(&make_cert_msg(), &mut record).unwrap();
    hs.handle_message(&make_ske_msg(), &mut record).unwrap();
    let result = hs.handle_message(&make_shd_msg(), &mut record).unwrap();

    if let Tls12HandshakeResult::SendToServer(bytes) = result {
        // ChangeCipherSpec record has content type 0x14
        assert!(bytes.windows(1).any(|w| w == [0x14]), "CCS byte 0x14 not found in send bytes");
    } else {
        panic!("Expected SendToServer");
    }
}

#[test]
fn handle_server_ccs_transitions_to_expect_finished() {
    let mut hs = make_handshake();
    let mut record = Tls12RecordLayer::new();

    hs.handle_message(&make_cert_msg(), &mut record).unwrap();
    hs.handle_message(&make_ske_msg(), &mut record).unwrap();
    hs.handle_message(&make_shd_msg(), &mut record).unwrap();

    // After SendToServer, we should be in ExpectServerChangeCipherSpec
    // handle_server_ccs should succeed
    hs.handle_server_ccs().unwrap();
}
