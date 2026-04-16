fn main() {
    if let Err(e) = viv::repl::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
