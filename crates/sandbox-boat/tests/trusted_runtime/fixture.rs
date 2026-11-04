//! Disposable real-UID fixtures for isolated trusted entry points.

use std::{
    collections::BTreeMap,
    fs,
    ops::{Deref, DerefMut},
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const BOOTSTRAP: &str = include_str!("../../src/trusted_runtime/helpers/bootstrap.py");
const LOADER: &str = include_str!("../../src/trusted_runtime/helpers/loader.py");

pub struct Fixture {
    pub temporary: TempDir,
    pub home: PathBuf,
    pub state: PathBuf,
    pub workspace: PathBuf,
    pub controller: String,
}

pub struct Attempt<'a> {
    pub fixture: &'a Fixture,
    pub id: String,
    pub digest: String,
}

impl Fixture {
    pub fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o755)).unwrap();
        let home = temporary.path().join("controller");
        let state = temporary.path().join("state");
        let workspace = temporary.path().join("workspace");
        fs::create_dir(&home).unwrap();
        fs::set_permissions(&home, fs::Permissions::from_mode(0o700)).unwrap();
        let controller = String::from_utf8(Command::new("id").arg("-un").output().unwrap().stdout)
            .unwrap()
            .trim()
            .to_owned();
        assert!(
            Command::new("sudo")
                .args(["-n", "chown", "root:root"])
                .arg(temporary.path())
                .status()
                .unwrap()
                .success()
        );
        Self {
            temporary,
            home,
            state,
            workspace,
            controller,
        }
    }
    pub fn attempt(&self, manifest: Value) -> Attempt<'_> {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let output = root_python(BOOTSTRAP)
            .args([
                self.home.to_str().unwrap(),
                self.state.to_str().unwrap(),
                self.workspace.to_str().unwrap(),
                &self.controller,
                "nobody",
                &id,
            ])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "bootstrap: {:?}",
            output.stderr
        );
        let sources = BTreeMap::from([
            (
                "common",
                STANDARD.encode(include_str!("../../src/trusted_runtime/helpers/common.py")),
            ),
            (
                "process",
                STANDARD.encode(include_str!("../../src/trusted_runtime/helpers/process.py")),
            ),
            (
                "files",
                STANDARD.encode(include_str!("../../src/trusted_runtime/helpers/files.py")),
            ),
            (
                "writer",
                STANDARD.encode(sandbox_runtime_assets::REGULAR_FILE_WRITE),
            ),
            (
                "cleanup",
                STANDARD.encode(sandbox_runtime_assets::REGULAR_FILE_CLEANUP),
            ),
        ]);
        let bundle = serde_json::to_vec(&json!({"version":1,"attempt":id,"workload_user":"nobody",
            "workspace":self.workspace,"manifest":manifest,"sources":sources}))
        .unwrap();
        let digest = digest(&bundle);
        fs::write(
            self.home
                .join(".sandbox-boat")
                .join(&id)
                .join("bundle.json"),
            bundle,
        )
        .unwrap();
        Attempt {
            fixture: self,
            id,
            digest,
        }
    }
    pub fn workload(&self, code: &str) -> Output {
        Command::new("sudo")
            .args([
                "-n",
                "-u",
                "nobody",
                "/usr/bin/env",
                "-i",
                "/usr/bin/python3",
                "-I",
                "-S",
                "-c",
                code,
            ])
            .output()
            .unwrap()
    }
    pub fn destination(&self, name: &str) -> PathBuf {
        self.workspace.join(name)
    }
}

impl Attempt<'_> {
    pub fn stage(&self, name: &str, bytes: &[u8]) {
        fs::write(
            self.fixture
                .home
                .join(".sandbox-boat")
                .join(&self.id)
                .join(name),
            bytes,
        )
        .unwrap();
    }
    pub fn command(&self, action: &str) -> Command {
        let mut command = root_python(LOADER);
        command.args([
            self.fixture.home.to_str().unwrap(),
            self.fixture.state.to_str().unwrap(),
            &self.fixture.controller,
            &self.id,
            &self.digest,
            action,
        ]);
        command
    }
    pub fn run(&self, action: &str) -> Output {
        self.command(action).output().unwrap()
    }
    pub fn write_manifest(fixture: &Fixture, path: &str, bytes: &[u8]) -> Value {
        json!({"operation":"write","root":fixture.workspace,"path":path,
            "expected_size":bytes.len(),"expected_digest":digest(bytes),"retain_fence":true})
    }
    pub fn state_path(&self, name: &str) -> PathBuf {
        self.fixture.state.join(&self.id).join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        assert!(
            Command::new("sudo")
                .args(["-n", "rm", "-rf", "--"])
                .arg(self.temporary.path())
                .status()
                .unwrap()
                .success()
        );
    }
}

pub fn root_python(source: &str) -> Command {
    let mut command = Command::new("sudo");
    command.args([
        "-n",
        "/usr/bin/env",
        "-i",
        "/usr/bin/python3",
        "-I",
        "-S",
        "-c",
        source,
    ]);
    command
}

pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub struct Supervisor<'a, 'b> {
    attempt: &'a Attempt<'b>,
    child: Child,
}

impl<'a, 'b> Attempt<'b> {
    pub fn supervise(&'a self) -> Supervisor<'a, 'b> {
        Supervisor {
            attempt: self,
            child: self
                .command("supervise")
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        }
    }
}
impl Deref for Supervisor<'_, '_> {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl DerefMut for Supervisor<'_, '_> {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}
impl Drop for Supervisor<'_, '_> {
    fn drop(&mut self) {
        let _ = self.attempt.run("terminate");
        let _ = self.attempt.run("ack");
        let _ = self.child.wait();
    }
}
