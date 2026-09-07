pub const EXEC_TEMPLATE: &str = r###"// -----------------------------------------------------------------------------
// Safe Subprocess Execution Engine (No Shell, Timeout Enforced)
// -----------------------------------------------------------------------------
pub struct ExecOptions<'a> {
    pub command: &'a str,
    pub args: &'a [String],
    pub workdir: Option<&'a str>,
    pub envs: &'a [(String, String)],
    pub timeout: Duration,
}

pub fn execute_subprocess(opts: ExecOptions) -> Result<Output, String> {
    mcp_log!("[EXEC] Starting '{}' with args {:?}", opts.command, opts.args);
    let mut cmd = Command::new(opts.command);
    cmd.args(opts.args);

    if let Some(dir) = opts.workdir {
        cmd.current_dir(dir);
    }

    for (k, v) in opts.envs {
        cmd.env(k, v);
    }

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let start = std::time::Instant::now();
    let child = cmd.spawn().map_err(|e| format!("Failed to spawn '{}': {}", opts.command, e))?;
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let res = child.wait_with_output();
        let _ = tx.send(res);
    });

    match rx.recv_timeout(opts.timeout) {
        Ok(Ok(output)) => {
            let elapsed = start.elapsed();
            mcp_log!("[EXEC] Exited with status {:?} in {}ms", output.status, elapsed.as_millis());
            Ok(output)
        }
        Ok(Err(e)) => Err(format!("I/O error during subprocess execution: {}", e)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(format!("Process exceeded execution timeout of {}ms", opts.timeout.as_millis()))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("Process supervisor worker thread terminated unexpectedly".to_string())
        }
    }
}
"###;
