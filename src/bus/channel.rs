use crate::bus::{AgentEvent, AgentMessage};
use crate::core::runtime::channel::{AsyncReceiver, NotifySender, async_channel};
use std::sync::mpsc;

/// Parent/holder side -- sends events to child agent, receives messages from it.
pub struct AgentHandle {
    pub tx: NotifySender<AgentEvent>,
    pub rx: mpsc::Receiver<AgentMessage>,
}

/// Child agent side -- receives events from parent, sends messages to parent.
pub struct AgentEndpoint {
    pub rx: AsyncReceiver<AgentEvent>,
    pub tx: mpsc::Sender<AgentMessage>,
}

/// Create a bidirectional channel for agent-to-agent communication.
pub fn agent_channel() -> (AgentHandle, AgentEndpoint) {
    let (event_tx, event_rx) = async_channel();
    let (msg_tx, msg_rx) = mpsc::channel();
    (
        AgentHandle {
            tx: event_tx,
            rx: msg_rx,
        },
        AgentEndpoint {
            rx: event_rx,
            tx: msg_tx,
        },
    )
}
