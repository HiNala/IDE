//! Slow / disk-heavy: run manually with  
//! `cargo test -p editor-io --test stress_large_file -- --ignored --nocapture`

use std::io::Write;

use editor_core::{BytePos, LineEnding};
use editor_io::{load_file_sync, save_file_sync, Encoding};

const HALF_GIB: u64 = 512 * 1024 * 1024;

#[test]
#[ignore]
fn load_half_gb_roundtrip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let p = dir.path().join("big.txt");
    {
        let mut f = std::fs::File::create(&p).expect("create");
        let chunk = vec![b'x'; 1024 * 1024];
        let mut w = 0u64;
        while w < HALF_GIB {
            f.write_all(&chunk).expect("write");
            w += chunk.len() as u64;
        }
    }
    let loaded = load_file_sync(&p).expect("load");
    assert!(loaded.byte_size >= HALF_GIB);
    let mut b = loaded.buffer;
    b.insert(BytePos(0), "EDIT").expect("insert");
    let snap = b.snapshot();
    save_file_sync(&p, &snap, LineEnding::Lf, Encoding::Utf8).expect("save");
    let _ = load_file_sync(&p).expect("reload");
    let _ = std::fs::remove_file(&p);
}
