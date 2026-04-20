use viv::agent::protocol::{AgentEvent, AgentMessage};

#[test]
fn agent_event_input_holds_string() {
    let event = AgentEvent::Input("hello".to_string());
    match event {
        AgentEvent::Input(s) => assert_eq!(s, "hello"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn agent_message_text_chunk_holds_string() {
    let msg = AgentMessage::TextChunk("world".to_string());
    match msg {
        AgentMessage::TextChunk(s) => assert_eq!(s, "world"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn agent_message_permission_request_holds_fields() {
    let msg = AgentMessage::PermissionRequest {
        tool: "bash".to_string(),
        input: "ls -la".to_string(),
    };
    match msg {
        AgentMessage::PermissionRequest { tool, input } => {
            assert_eq!(tool, "bash");
            assert_eq!(input, "ls -la");
        }
        _ => panic!("wrong variant"),
    }
}

#[test]
fn channel_sends_events_between_threads() {
    use std::sync::mpsc::channel;
    let (tx, rx) = channel::<AgentEvent>();
    tx.send(AgentEvent::Input("test".to_string())).unwrap();
    match rx.recv().unwrap() {
        AgentEvent::Input(s) => assert_eq!(s, "test"),
        _ => panic!("wrong event"),
    }
}
