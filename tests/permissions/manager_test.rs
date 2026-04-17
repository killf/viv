use viv::core::json::JsonValue;
use viv::permissions::PermissionManager;
use viv::tools::{PermissionLevel, Tool};

struct FakeTool { level: PermissionLevel }

impl Tool for FakeTool {
    fn name(&self) -> &str { "fake" }
    fn description(&self) -> &str { "" }
    fn input_schema(&self) -> JsonValue { JsonValue::Null }
    fn execute(&self, _: &JsonValue) -> viv::Result<String> { Ok("ok".into()) }
    fn permission_level(&self) -> PermissionLevel { self.level.clone() }
}

#[test]
fn readonly_always_allowed_without_asking() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::ReadOnly };
    let mut asked = false;
    let allowed = pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked = true; true });
    assert!(allowed);
    assert!(!asked);
}

#[test]
fn write_asks_on_first_call() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::Write };
    let mut asked = false;
    let allowed = pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked = true; true });
    assert!(asked);
    assert!(allowed);
}

#[test]
fn write_not_asked_again_after_grant() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::Write };
    pm.check(&tool, &JsonValue::Null, &mut |_, _| true);
    let mut asked_again = false;
    let allowed = pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked_again = true; false });
    assert!(!asked_again);
    assert!(allowed);
}

#[test]
fn denied_not_remembered_next_call_asks_again() {
    let mut pm = PermissionManager::default();
    let tool = FakeTool { level: PermissionLevel::Execute };
    pm.check(&tool, &JsonValue::Null, &mut |_, _| false);
    let mut asked_again = false;
    pm.check(&tool, &JsonValue::Null, &mut |_, _| { asked_again = true; false });
    assert!(asked_again);
}
