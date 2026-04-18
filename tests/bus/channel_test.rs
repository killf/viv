use viv::bus::channel::agent_channel;
use viv::bus::{AgentEvent, AgentMessage};
use viv::core::runtime::block_on_local;

#[test]
fn agent_channel_send_event_receive_in_endpoint() {
    let (handle, endpoint) = agent_channel().unwrap();
    handle.tx.send(AgentEvent::Input("hello".into())).unwrap();
    let event = block_on_local(Box::pin(endpoint.rx.recv())).unwrap();
    match event {
        AgentEvent::Input(s) => assert_eq!(s, "hello"),
        other => panic!("expected Input, got {:?}", other),
    }
}

#[test]
fn agent_channel_send_message_receive_in_handle() {
    let (handle, endpoint) = agent_channel().unwrap();
    endpoint
        .tx
        .send(AgentMessage::TextChunk("chunk".into()))
        .unwrap();
    match handle.rx.try_recv() {
        Ok(AgentMessage::TextChunk(s)) => assert_eq!(s, "chunk"),
        other => panic!("expected TextChunk, got {:?}", other),
    }
}

#[test]
fn agent_channel_bidirectional_permission_flow() {
    let (handle, endpoint) = agent_channel().unwrap();
    endpoint
        .tx
        .send(AgentMessage::PermissionRequest {
            tool: "Bash".into(),
            input: "command=ls".into(),
        })
        .unwrap();
    match handle.rx.try_recv() {
        Ok(AgentMessage::PermissionRequest { tool, .. }) => assert_eq!(tool, "Bash"),
        other => panic!("expected PermissionRequest, got {:?}", other),
    }
    handle
        .tx
        .send(AgentEvent::PermissionResponse(true))
        .unwrap();
    let event = block_on_local(Box::pin(endpoint.rx.recv())).unwrap();
    match event {
        AgentEvent::PermissionResponse(true) => {}
        other => panic!("expected PermissionResponse(true), got {:?}", other),
    }
}
