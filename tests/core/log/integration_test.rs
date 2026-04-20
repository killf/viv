//! Integration test: verify log file is written correctly.

use std::fs;
use std::io::Read;
use viv::log::{Level, init};

/// Serialize init() calls so flushers don't overwrite each other mid-test.
static INIT_MTX: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tmp_path(name: &str) -> String {
    let mut p = std::env::temp_dir();
    p.push(name);
    p.to_string_lossy().into_owned()
}

fn run_with_lock(path: &str, level: Level, blk: impl Fn()) {
    let _guard = INIT_MTX.lock().unwrap();
    let _ = fs::remove_file(path);
    init(path, level).expect("init");
    std::thread::sleep(std::time::Duration::from_millis(300));
    blk();
    std::thread::sleep(std::time::Duration::from_millis(2000));
}

#[test]
fn log_file_written() {
    let path = tmp_path("viv_it_a.log");
    run_with_lock(&path, Level::Debug, || {
        viv::info!("msg1 {}", 1);
        viv::debug!("msg2 {}", 2);
        viv::error!("msg3 {}", 3);
    });

    let mut c = String::new();
    let mut f = fs::File::open(&path).expect("open log");
    f.read_to_string(&mut c).expect("read log");

    assert!(c.contains("msg1 1"), "missing msg1, got: {:?}", &c);
    assert!(c.contains("msg2 2"), "missing msg2");
    assert!(c.contains("msg3 3"), "missing msg3");
    assert!(c.contains("INFO"), "missing INFO");
    assert!(c.contains("DEBUG"), "missing DEBUG");
    assert!(c.contains("ERROR"), "missing ERROR");
}

#[test]
fn log_level_filter() {
    let path = tmp_path("viv_it_b.log");
    run_with_lock(&path, Level::Warn, || {
        viv::info!("msg_x");
        viv::error!("msg_y");
    });

    let mut c = String::new();
    let mut f = fs::File::open(&path).expect("open log");
    f.read_to_string(&mut c).expect("read log");

    assert!(!c.contains("msg_x"), "info leaked, got: {:?}", &c);
    assert!(c.contains("msg_y"), "missing error");
}
