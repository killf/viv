use std::sync::mpsc::channel;
use std::thread;
use viv::agent::agent::{Agent, AgentConfig};
use viv::bus::{AgentEvent, AgentMessage};
use viv::bus::terminal::TerminalUI;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> viv::Result<()> {
    let (event_tx, event_rx) = channel::<AgentEvent>();
    let (msg_tx, msg_rx) = channel::<AgentMessage>();

    let config = AgentConfig::default();
    let agent = Agent::new(config, event_rx, msg_tx)?;

    let handle = thread::spawn(move || agent.run());

    TerminalUI::new(event_tx, msg_rx)?.run()?;

    handle.join().unwrap_or(Ok(()))
}
