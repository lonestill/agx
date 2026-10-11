use std::process::{Command, Stdio};
use colored::*;
use crate::account::{read_active_token, write_active_token, load_account};

pub fn run_with_account(account_name: &str, command: &[String]) -> Result<i32, String> {
    if command.is_empty() {
        return Err("No command specified after '--'".to_string());
    }

    // 1. Save original token
    let original_token = read_active_token().map_err(|e| format!("Could not read active token: {}", e))?;

    // 2. Load target account
    let target_token = load_account(account_name).map_err(|e| format!("Could not load account '{}': {}", account_name, e))?;

    // 3. Atomically switch to target account
    write_active_token(&target_token).map_err(|e| format!("Could not apply temporary token: {}", e))?;

    println!("{} Executing under account '{}': {:?}", "[run]".cyan().bold(), account_name.bold(), command);

    // 4. Run child process
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]);
    child.stdin(Stdio::inherit());
    child.stdout(Stdio::inherit());
    child.stderr(Stdio::inherit());

    let exit_status = child.status();

    // 5. Always restore original token
    let restore_res = write_active_token(&original_token);

    match (exit_status, restore_res) {
        (Ok(status), Ok(_)) => {
            let code = status.code().unwrap_or(0);
            Ok(code)
        }
        (Err(e), _) => Err(format!("Failed to execute command: {}", e)),
        (_, Err(e)) => Err(format!("Warning: child finished, but failed to restore original token: {}", e)),
    }
}
