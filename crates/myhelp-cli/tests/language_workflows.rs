use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn pack_directory() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("examples/language-workflows")
}

fn myhelp(arguments: &[&str], pages: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_myhelp"))
        .arg("--pages-dir")
        .arg(pages)
        .args(arguments)
        .output()
        .expect("run myhelp")
}

#[test]
fn starter_pack_lists_shows_and_validates_every_manifest_page() {
    let pack = pack_directory();
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(pack.join("manifest.json")).expect("read starter-pack manifest"),
    )
    .expect("parse starter-pack manifest");
    let pages = manifest["pages"].as_array().expect("manifest pages array");

    let listed = myhelp(&["list", "--json"], &pack);
    assert!(
        listed.status.success(),
        "{}",
        String::from_utf8_lossy(&listed.stderr)
    );
    let listed: Value = serde_json::from_slice(&listed.stdout).expect("list JSON");
    let listed_topics: Vec<&str> = listed
        .as_array()
        .expect("listed pages")
        .iter()
        .map(|page| page["topic"].as_str().expect("listed topic"))
        .collect();
    let expected_topics: Vec<&str> = pages
        .iter()
        .map(|page| page["topic"].as_str().expect("manifest topic"))
        .collect();
    assert_eq!(listed_topics, expected_topics);

    for page in pages {
        let topic = page["topic"].as_str().expect("manifest topic");
        let path = pack.join(page["file"].as_str().expect("manifest file"));
        let expected_content = fs::read_to_string(&path).expect("read starter-pack page");

        let shown = myhelp(&["show", topic], &pack);
        assert!(
            shown.status.success(),
            "{topic}: {}",
            String::from_utf8_lossy(&shown.stderr)
        );
        assert_eq!(
            String::from_utf8(shown.stdout).expect("UTF-8 show output"),
            expected_content,
            "{topic} must remain readable without rewriting"
        );

        let validated = myhelp(
            &[
                "tldr",
                "validate",
                path.to_str().expect("UTF-8 starter-pack path"),
                "--topic",
                topic,
                "--json",
            ],
            &pack,
        );
        assert!(
            validated.status.success(),
            "{topic}: {}",
            String::from_utf8_lossy(&validated.stderr)
        );
        let report: Value =
            serde_json::from_slice(&validated.stdout).expect("validation report JSON");
        assert_eq!(report["valid"], true, "{topic}: {report}");
    }
}
