// Integration test: `fsn init` wizard creates the expected file structure.
//
// Uses a temp directory and pipes stdin answers into the binary, then verifies
// that the generated project + host TOML files exist and have expected content.

use std::io::Write;
use std::process::{Command, Stdio};

const WIZARD_INPUT: &str = "\
Test Project\n\
test.example.com\n\
admin@example.com\n\
203.0.113.10\n\
\n\
hetzner\n\
letsencrypt\n\
n\n\
";

fn fsn_bin() -> std::path::PathBuf {
    // CARGO_BIN_EXE_fsn is set by Cargo when running integration tests.
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_fsn"))
}

#[test]
fn init_wizard_creates_project_skeleton() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    let mut child = Command::new(fsn_bin())
        .args(["--root", root.to_str().unwrap(), "init"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fsn init");

    // Write wizard answers to stdin.
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(WIZARD_INPUT.as_bytes()).expect("write stdin");
    }

    let output = child.wait_with_output().expect("wait for fsn init");

    // The wizard should succeed (no modules dir → phases 2+3 skipped, deploy skipped).
    assert!(
        output.status.success(),
        "fsn init exited with {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Project TOML should exist.
    let proj_toml = root
        .join("projects")
        .join("test-project")
        .join("test-project.project.toml");
    assert!(proj_toml.exists(), "project TOML not created: {:?}", proj_toml);

    let proj_content = std::fs::read_to_string(&proj_toml).expect("read project TOML");
    assert!(proj_content.contains("test.example.com"), "domain not in project TOML");
    assert!(proj_content.contains("admin@example.com"), "email not in project TOML");

    // Host TOML should exist.
    let host_toml = root.join("hosts").join("test-project.host.toml");
    assert!(host_toml.exists(), "host TOML not created: {:?}", host_toml);

    let host_content = std::fs::read_to_string(&host_toml).expect("read host TOML");
    assert!(host_content.contains("203.0.113.10"), "IP not in host TOML");
    assert!(host_content.contains("hetzner"), "DNS provider not in host TOML");

    // Vault stub should exist.
    let vault = root.join("projects").join("test-project").join("vault.toml");
    assert!(vault.exists(), "vault.toml not created: {:?}", vault);
}

#[test]
fn init_wizard_finds_existing_project_and_skips_skeleton() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    // Pre-create a project skeleton.
    let proj_dir = root.join("projects").join("existing");
    std::fs::create_dir_all(&proj_dir).unwrap();
    std::fs::write(
        proj_dir.join("existing.project.toml"),
        "[project]\nname = \"existing\"\ndomain = \"existing.example.com\"\n\
         [project.contact]\nemail = \"a@b.com\"\nacme_email = \"a@b.com\"\n\
         [load.services]\n",
    )
    .unwrap();
    let hosts_dir = root.join("hosts");
    std::fs::create_dir_all(&hosts_dir).unwrap();
    std::fs::write(
        hosts_dir.join("existing.host.toml"),
        "[host]\nname = \"existing\"\nip = \"1.2.3.4\"\n\
         [proxy.zentinel]\nservice_class = \"proxy/zentinel\"\n\
         [proxy.zentinel.load.plugins]\ndns = \"hetzner\"\nacme = \"letsencrypt\"\nacme_email = \"a@b.com\"\n",
    )
    .unwrap();

    // Answer "n" to deploy prompt — wizard finds existing project, skips skeleton creation.
    let mut child = Command::new(fsn_bin())
        .args(["--root", root.to_str().unwrap(), "init"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn fsn init");

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(b"n\n").expect("write stdin");
    }

    let output = child.wait_with_output().expect("wait");
    assert!(
        output.status.success(),
        "fsn init with existing project failed\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    // Original project file must still be intact.
    assert!(proj_dir.join("existing.project.toml").exists());
}
