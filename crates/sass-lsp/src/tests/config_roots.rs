#[test]
fn compute_allowed_roots_keeps_every_workspace_folder() {
    use std::path::PathBuf;

    let folders = vec![PathBuf::from("/ws/a"), PathBuf::from("/ws/b")];
    let config = crate::config::SassAnalyzerConfig {
        load_paths: vec!["styles".to_owned()],
        ..Default::default()
    };

    let roots = crate::compute_allowed_roots(&folders, &config);

    assert!(roots.contains(&PathBuf::from("/ws/a")));
    assert!(
        roots.contains(&PathBuf::from("/ws/b")),
        "secondary workspace folders must survive a configuration change"
    );
    assert!(roots.contains(&PathBuf::from("/ws/a/styles")));
}

#[test]
fn compute_allowed_roots_is_empty_without_folders() {
    let config = crate::config::SassAnalyzerConfig::default();
    assert!(crate::compute_allowed_roots(&[], &config).is_empty());
}
