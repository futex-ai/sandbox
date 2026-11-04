//! Actual isolated child identity and exact argv/output before completion.

use std::{
    fs,
    io::{BufRead, BufReader},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};

use super::fixture::Fixture;

#[test]
fn supervisor_preserves_exact_argv_binary_output_and_actual_workload_identity() {
    let fixture = Fixture::new();
    let script = "import os,sys,json,subprocess;assert os.getuid()==65534;assert os.getgroups()==[];assert 'NoNewPrivs:\t1' in open('/proc/self/status').read();assert subprocess.run(['/usr/bin/sudo','-n','/usr/bin/id'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode!=0;sys.stdout.buffer.write(bytes([0,255,10])+json.dumps(sys.argv[1:],separators=(',',':')).encode());sys.stdout.flush();sys.stderr.buffer.write(bytes([254,0]));sys.stderr.flush()";
    let long = "x".repeat(96 * 1024);
    let large_environment = "y".repeat(60 * 1024);
    let args = vec!["-I", "-S", "-c", script, "a b'$(touch unexpected)\n", &long];
    let attempt = fixture.attempt(json!({"operation":"process","command":"/usr/bin/python3",
        "args":args,"cwd":fixture.workspace,"envs":{"EXACT":"value","LARGE":large_environment},"timeout_ms":10000}));
    let mut child = attempt.supervise();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    let hello: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(hello["kind"], "hello");
    assert_eq!(attempt.run("release").status.code(), Some(0));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut pid = 0;
    loop {
        line.clear();
        assert_ne!(output.read_line(&mut line).unwrap(), 0);
        let frame: Value = serde_json::from_str(&line).unwrap();
        match frame["kind"].as_str().unwrap() {
            "started" => {
                pid = frame["pid"].as_u64().unwrap();
            }
            "stdout" => stdout.extend(STANDARD.decode(frame["data"].as_str().unwrap()).unwrap()),
            "stderr" => stderr.extend(STANDARD.decode(frame["data"].as_str().unwrap()).unwrap()),
            "exited" => {
                assert_eq!(frame["exit_code"], 0);
                assert_eq!(frame["exited"], true);
            }
            "success" => break,
            _ => panic!("unexpected helper frame"),
        }
    }
    assert_ne!(pid, 0);
    assert_ne!(pid, u64::from(child.id()));
    assert_eq!(
        stdout,
        [
            b"\0\xff\n".as_slice(),
            serde_json::to_vec(&vec![args[4], args[5]])
                .unwrap()
                .as_slice()
        ]
        .concat()
    );
    assert_eq!(stderr, b"\xfe\0");
    let record = super::fixture::root_python(
        "import sys;sys.stdout.buffer.write(open(sys.argv[1],'rb').read())",
    )
    .arg(attempt.state_path("child.json"))
    .output()
    .unwrap();
    let record: Value = serde_json::from_slice(&record.stdout).unwrap();
    assert_eq!(record["pid"], pid);
    assert_eq!(attempt.run("ack").status.code(), Some(0));
    assert_eq!(child.wait().unwrap().code(), Some(0));
    assert_eq!(attempt.run("clean").status.code(), Some(0));
    assert!(!fixture.destination("unexpected").exists());
    assert_eq!(
        fs::read_dir(fixture.home.join(".sandbox-boat"))
            .unwrap()
            .count(),
        0
    );
}

#[test]
fn trusted_cancel_terminates_the_live_workload_group_and_duplicate_launch_is_fenced() {
    let fixture = Fixture::new();
    let attempt = fixture.attempt(json!({"operation":"process","command":"/usr/bin/python3",
        "args":["-I","-S","-c","import signal;signal.pause()"],"cwd":fixture.workspace,"envs":{},"timeout_ms":10000}));
    let mut child = attempt.supervise();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&line).unwrap()["kind"],
        "hello"
    );
    assert_eq!(attempt.run("release").status.code(), Some(0));
    line.clear();
    output.read_line(&mut line).unwrap();
    let started: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(started["kind"], "started");
    let pid = started["pid"].as_u64().unwrap();
    assert_eq!(attempt.run("supervise").status.code(), Some(49));
    assert_eq!(attempt.run("terminate").status.code(), Some(0));
    let _ = attempt.run("ack");
    assert_eq!(child.wait().unwrap().code(), Some(49));
    assert!(!std::path::Path::new(&format!("/proc/{pid}")).exists());
}
