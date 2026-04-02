use std::fs;
use std::path::PathBuf;

use anyhow::Result;

use crate::kubeconfig;

struct SessionInfo {
    pid: u32,
    context: String,
    namespace: String,
    cwd: String,
}

fn guardian_dir() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    std::env::temp_dir().join(format!("kubie-guardian-{uid}"))
}

fn read_sessions() -> Result<Vec<SessionInfo>> {
    let tracking_file = guardian_dir().join("sessions");
    let contents = match fs::read_to_string(&tracking_file) {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let mut sessions = Vec::new();

    for line in contents.lines() {
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() != 3 {
            continue;
        }

        let pid: u32 = match parts[0].parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        // Check if the process is still alive.
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            continue;
        }

        let config_path = parts[1];

        // Read context and namespace from the kubeconfig.
        let (context, namespace) = match fs::read_to_string(config_path)
            .ok()
            .and_then(|yaml| serde_yaml::from_str::<kubeconfig::KubeConfig>(&yaml).ok())
        {
            Some(config) => {
                let ctx = config.current_context.unwrap_or_default();
                let ns = config
                    .contexts
                    .first()
                    .and_then(|c| c.context.namespace.clone())
                    .unwrap_or_else(|| "default".into());
                (ctx, ns)
            }
            None => continue,
        };

        // Read cwd from /proc.
        let cwd = fs::read_link(format!("/proc/{pid}/cwd"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "?".into());

        sessions.push(SessionInfo {
            pid,
            context,
            namespace,
            cwd,
        });
    }

    Ok(sessions)
}

pub fn sessions() -> Result<()> {
    let sessions = read_sessions()?;

    if sessions.is_empty() {
        println!("No active kubie sessions.");
        return Ok(());
    }

    // Calculate column widths.
    let pid_w = sessions
        .iter()
        .map(|s| s.pid.to_string().len())
        .max()
        .unwrap_or(3)
        .max(3);
    let ctx_w = sessions.iter().map(|s| s.context.len()).max().unwrap_or(7).max(7);
    let ns_w = sessions.iter().map(|s| s.namespace.len()).max().unwrap_or(9).max(9);

    println!("{:<pid_w$}  {:<ctx_w$}  {:<ns_w$}  CWD", "PID", "CONTEXT", "NAMESPACE");

    for s in &sessions {
        println!(
            "{:<pid_w$}  {:<ctx_w$}  {:<ns_w$}  {}",
            s.pid, s.context, s.namespace, s.cwd
        );
    }

    Ok(())
}
