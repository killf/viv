use std::sync::mpsc::channel;
use std::thread;
use viv::agent::agent::{Agent, AgentConfig};
use viv::bus::AgentMessage;
use viv::bus::terminal::TerminalUI;
use viv::core::runtime::channel::async_channel;
use viv::core::runtime::block_on_local;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> viv::Result<()> {
    let (event_tx, event_rx) = async_channel();
    let (msg_tx, msg_rx) = channel::<AgentMessage>();

    let config = AgentConfig::default();

    // Agent 完全在 async 上下文中创建和运行
    // 在独立线程中避免 Send 约束（Agent 持有 OpenSSL raw pointer）
    let handle = thread::spawn(move || {
        block_on_local(Box::pin(async move {
            let agent = Agent::new(config, event_rx, msg_tx).await?;
            agent.run().await
        }))
    });

    TerminalUI::new(event_tx, msg_rx)?.run()?;

    handle.join().unwrap_or(Ok(()))
}
