use std::path::Path;

use rime_cli::{natural_sort_dng_paths, resolve_graph_config, Cli, Command, RenderOptions};

#[test]
fn cli_exposes_v013_headless_commands() {
    let cli = Cli::try_parse_from(["rime-frameforge", "inspect", "input.dng"])
        .expect("inspect command must parse");
    assert!(matches!(cli.command, Command::Inspect { .. }));

    let cli = Cli::try_parse_from([
        "rime-frameforge",
        "render",
        "input.dng",
        "--output",
        "out.png",
    ])
    .expect("render command must parse");
    assert!(matches!(cli.command, Command::Render { .. }));
}

#[test]
fn dry_run_and_json_progress_are_explicit_render_options() {
    let cli = Cli::try_parse_from([
        "rime-frameforge",
        "render-sequence",
        "frame02.dng",
        "--output",
        "out.mp4",
        "--dry-run",
        "--json",
        "--progress",
        "jsonl",
    ])
    .expect("sequence options must parse");
    let Command::RenderSequence { options, .. } = cli.command else {
        panic!("expected sequence");
    };
    assert!(options.dry_run);
    assert!(options.json);
    assert_eq!(options.progress.as_deref(), Some("jsonl"));
}

#[test]
fn sequence_selection_scans_only_the_selected_dng_parent_and_natural_sorts() {
    let mut paths = vec![
        Path::new("/capture/P10.dng").to_owned(),
        Path::new("/capture/P2.DNG").to_owned(),
        Path::new("/capture/P1.dng").to_owned(),
        Path::new("/other/P0.dng").to_owned(),
    ];
    natural_sort_dng_paths(&mut paths, Path::new("/capture/P2.DNG"));
    assert_eq!(
        paths
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy())
            .collect::<Vec<_>>(),
        ["P1.dng", "P2.DNG", "P10.dng"]
    );
}

#[test]
fn resolved_config_reuses_normal_manifest_and_fixed_ring() {
    let resolved = resolve_graph_config(None).expect("default graph resolves");
    assert_eq!(resolved.graph_id, "normal");
    assert_eq!(resolved.ring_capacity, 2);
    assert_eq!(
        resolved.manifest_hash,
        rime_isp::build_normal_manifest().manifest_hash
    );
}

#[test]
fn render_options_default_to_non_destructive_output() {
    let options = RenderOptions::default();
    assert!(!options.dry_run);
    assert_eq!(options.progress.as_deref(), Some("jsonl"));
}
