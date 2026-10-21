//! Each profile must produce the same graph through full and incremental paths.
use std::fs;
use tempfile::TempDir;
use tokensave::config::{load_config, save_config};
use tokensave::cpu::CpuUsage;
use tokensave::tokensave::TokenSave;

#[tokio::test]
async fn profiles_preserve_full_and_incremental_graphs() {
    let mut expected = None;
    for (profile, memory) in [CpuUsage::Low, CpuUsage::Balanced, CpuUsage::Performance]
        .into_iter()
        .flat_map(|cpu| {
            [
                tokensave::memory::MemoryUsage::Low,
                tokensave::memory::MemoryUsage::Balanced,
                tokensave::memory::MemoryUsage::Performance,
            ]
            .map(|memory| (cpu, memory))
        })
    {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.rs"), "pub fn a() -> u32 { 1 }\n").unwrap();
        fs::write(dir.path().join("b.rs"), "pub fn b() -> u32 { a() }\n").unwrap();
        fs::write(dir.path().join("notes.md"), "artifact\n").unwrap();
        // Cross more than one bounded fallback slice (balanced permits two).
        for i in 0..5 {
            fs::write(
                dir.path().join(format!("extra{i}.rs")),
                format!("pub fn extra{i}() {{ a(); }}\n"),
            )
            .unwrap();
        }
        drop(TokenSave::init(dir.path()).await.unwrap());
        let mut config = load_config(dir.path()).unwrap();
        config.cpu_usage = profile;
        config.memory_usage = memory;
        save_config(dir.path(), &config).unwrap();
        // Reinitializing must honor the same on-disk selection as opening.
        drop(TokenSave::init(dir.path()).await.unwrap());
        assert_eq!(load_config(dir.path()).unwrap().cpu_usage, profile);
        assert_eq!(load_config(dir.path()).unwrap().memory_usage, memory);
        let cg = TokenSave::open(dir.path()).await.unwrap();
        cg.index_all().await.unwrap();
        assert_eq!(cg.db().get_all_files().await.unwrap().len(), 8);
        // Compare stable graph identities, not timing or insertion order.
        let snapshot = || async {
            let mut nodes = cg
                .db()
                .get_all_nodes()
                .await
                .unwrap()
                .into_iter()
                .map(|n| (n.id, n.name, n.file_path))
                .collect::<Vec<_>>();
            let mut edges = cg
                .db()
                .get_all_edges()
                .await
                .unwrap()
                .into_iter()
                .map(|e| (e.source, e.target, format!("{:?}", e.kind)))
                .collect::<Vec<_>>();
            nodes.sort();
            edges.sort();
            (nodes, edges)
        };
        let full = snapshot().await;
        assert!(!full.0.is_empty());
        assert!(!full.1.is_empty());
        if let Some(ref baseline) = expected {
            assert_eq!(&full, baseline);
        } else {
            expected = Some(full);
        }
        fs::write(
            dir.path().join("a.rs"),
            "pub fn a() -> u32 { 42 }\npub fn added() {}\n",
        )
        .unwrap();
        let changed = vec!["a.rs".to_string()];
        cg.sync_if_stale_silent(&changed).await.unwrap();
        let incremental = snapshot().await;
        cg.index_all().await.unwrap();
        assert_eq!(incremental, snapshot().await);
        fs::write(dir.path().join("b.rs"), "pub fn b() -> u32 { a() + 100 }\n").unwrap();
        let result = cg.sync().await.unwrap();
        assert_eq!(result.files_modified, 1);
        let incremental = snapshot().await;
        cg.index_all().await.unwrap();
        assert_eq!(incremental, snapshot().await);
        assert!(!dir.path().join(".tokensave").join("dirty").exists());
    }
}
