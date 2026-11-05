//! Collected and stateless execution share the exact framed runtime route.

use std::{collections::BTreeMap, time::Duration};

use futures_util::StreamExt;
use sandbox_interface::{
    BackendReadOnlyExecRequest, BackendRunProcessRequest, BackendStreamProcessRequest, Error,
    PROCESS_RUN_MAX_DEADLINE, ProcessStreamEvent, ProcessStreamOutcome, ReadOnlyExecOutput, Result,
    SandboxProcessOutput,
};

use crate::runtime::configured::BoatTrustedRuntime;

use super::{execution, settings};

pub(crate) async fn run(
    runtime: &BoatTrustedRuntime,
    request: BackendRunProcessRequest,
) -> Result<SandboxProcessOutput> {
    request.validate_execution_context()?;
    let request = BackendStreamProcessRequest {
        sandbox_provider_ref: request.sandbox_provider_ref,
        command: request.command,
        args: request.args,
        cwd: request.cwd,
        envs: request.envs,
        stdout_limit: request.stdout_limit,
        stderr_limit: request.stderr_limit,
        deadline: request.deadline,
        idle_timeout: request.deadline.max(Duration::from_nanos(1)),
    };
    settings::validate_common(&request)?;
    duration(request.deadline)?;
    if request.deadline.is_zero() {
        let mut check = request;
        check.deadline = Duration::from_nanos(1);
        settings::validate(&check)?;
        return Ok(SandboxProcessOutput::default());
    }
    let mut events = execution::stream(runtime, request, None).await?;
    let mut output = SandboxProcessOutput::default();
    while let Some(event) = events.next().await {
        match event {
            ProcessStreamEvent::Stdout(bytes) => output.stdout.extend(bytes),
            ProcessStreamEvent::Stderr(bytes) => output.stderr.extend(bytes),
            ProcessStreamEvent::Exited { exit_code, exited } => {
                output.exit_code = Some(exit_code);
                output.exited = exited;
            }
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::StdoutOverflow) => {
                output.stdout_overflowed = true
            }
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::StderrOverflow) => {
                output.stderr_overflowed = true
            }
            ProcessStreamEvent::Outcome(
                ProcessStreamOutcome::TransportFailure | ProcessStreamOutcome::ConsumerBackpressure,
            ) => return Err(runtime.unavailable()),
            _ => {}
        }
    }
    Ok(output)
}

pub(crate) async fn read_only(
    runtime: &BoatTrustedRuntime,
    request: BackendReadOnlyExecRequest,
) -> Result<ReadOnlyExecOutput> {
    let request = BackendStreamProcessRequest {
        sandbox_provider_ref: request.sandbox_provider_ref,
        command: request.executable,
        args: request.args,
        cwd: Some(request.cwd),
        envs: BTreeMap::new(),
        stdout_limit: request.output_limit,
        stderr_limit: request.output_limit,
        deadline: request.timeout,
        idle_timeout: request.timeout.max(Duration::from_nanos(1)),
    };
    settings::validate_common(&request)?;
    duration(request.deadline)?;
    if request.deadline.is_zero() {
        let mut check = request;
        check.deadline = Duration::from_nanos(1);
        settings::validate(&check)?;
        return Err(runtime.unavailable());
    }
    let combined = request.stdout_limit;
    let mut events = execution::stream(runtime, request, Some(combined)).await?;
    let mut bytes = Vec::new();
    let mut code = None;
    let mut completed = false;
    while let Some(event) = events.next().await {
        match event {
            ProcessStreamEvent::Stdout(part) | ProcessStreamEvent::Stderr(part) => {
                bytes.extend(part)
            }
            ProcessStreamEvent::Exited {
                exit_code,
                exited: true,
            } => code = Some(exit_code),
            ProcessStreamEvent::Outcome(ProcessStreamOutcome::Completed) => completed = true,
            ProcessStreamEvent::Outcome(
                ProcessStreamOutcome::StdoutOverflow | ProcessStreamOutcome::StderrOverflow,
            ) => return Err(Error::ReadOnlyOutputTooLarge),
            _ => {}
        }
    }
    match code {
        Some(exit_code) if completed => Ok(ReadOnlyExecOutput { bytes, exit_code }),
        _ => Err(runtime.unavailable()),
    }
}

fn duration(value: Duration) -> Result<()> {
    if value > PROCESS_RUN_MAX_DEADLINE {
        return Err(Error::InvalidSeconds {
            field: "deadline",
            minimum: 0,
            maximum: 300,
        });
    }
    Ok(())
}
