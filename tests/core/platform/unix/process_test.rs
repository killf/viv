use viv::core::platform::shell_command;

#[test]
fn shell_command_echo() {
    let output = shell_command("echo hello").output().expect("execute");
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "hello");
}
