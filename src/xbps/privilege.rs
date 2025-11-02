use std::process::Command;

use crate::types::CommandResult;

pub(crate) fn run_privileged_command(
    program: &str,
    args: &[&str],
) -> Result<CommandResult, String> {
    let output = Command::new("pkexec")
        .arg(program)
        .args(args)
        .output()
        .map_err(|err| format!("Failed to launch pkexec: {}", err))?;

    Ok(CommandResult {
        code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
    })
}
