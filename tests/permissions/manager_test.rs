use viv::permissions::PermissionManager;

#[test]
fn new_manager_allows_nothing() {
    let pm = PermissionManager::default();
    assert!(!pm.is_allowed("bash"));
    assert!(!pm.is_allowed("write"));
}

#[test]
fn grant_makes_tool_allowed() {
    let mut pm = PermissionManager::default();
    assert!(!pm.is_allowed("bash"));
    pm.grant("bash");
    assert!(pm.is_allowed("bash"));
}

#[test]
fn grant_is_tool_specific() {
    let mut pm = PermissionManager::default();
    pm.grant("bash");
    assert!(pm.is_allowed("bash"));
    assert!(!pm.is_allowed("write"));
}

#[test]
fn grant_is_idempotent() {
    let mut pm = PermissionManager::default();
    pm.grant("bash");
    pm.grant("bash");
    assert!(pm.is_allowed("bash"));
}
