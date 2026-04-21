//! Manual keychain verification — not run in CI.
//!
//! `cargo test -p editor-ai-provider keyring_roundtrip -- --ignored`

use editor_ai_provider::secrets::SecretStore;

#[test]
#[ignore = "requires a working OS secret service; run with --ignored before release"]
fn keyring_roundtrip() {
    let store = SecretStore::new();
    let account = format!("editor-m19-test-{}", std::process::id());
    store.set_key(&account, "test-secret-value").expect("set_key");
    let got = store.get_key(&account).expect("get_key");
    assert_eq!(got.as_deref(), Some("test-secret-value"));
    store.delete_key(&account).expect("delete_key");
}
