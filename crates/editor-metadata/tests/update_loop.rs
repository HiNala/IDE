//! MetadataUpdater with [`NoopSummarizer`] (no network).

use std::sync::Arc;

use editor_metadata::session::{SessionEvent, SessionLog};
use editor_metadata::store::MetadataStore;
use editor_metadata::summarizer::NoopSummarizer;
use editor_metadata::update::MetadataUpdater;
use tempfile::tempdir;

#[tokio::test]
async fn noop_updates_sidecar_for_committed_file() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let src = root.join("src").join("auth.rs");
    std::fs::create_dir_all(src.parent().unwrap()).unwrap();
    std::fs::write(&src, "fn x() {}\n").unwrap();

    let log = SessionLog {
        id: "sess-test-01".into(),
        committed_changes: vec![src.clone()],
        events: vec![SessionEvent::Note { text: "noop turn".into() }],
    };

    let store = Arc::new(MetadataStore::new(root.clone()));
    let sum: Arc<dyn editor_metadata::summarizer::Summarizer> = Arc::new(NoopSummarizer);
    let up = MetadataUpdater::new(Arc::clone(&store), sum, 2);

    let updated = up.update_for_session(&log).await.expect("update");
    assert_eq!(updated.len(), 1);

    let sc = store.load(&src).expect("load").expect("sidecar written");
    assert!(!sc.body.history.is_empty());
}
