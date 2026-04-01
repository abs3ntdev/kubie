use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use fs2::FileExt;

use self::detect::{detect, ShellKind};
use crate::kubeconfig::KubeConfig;
use crate::session::Session;
use crate::settings::Settings;
use crate::state;
use crate::vars;

mod bash;
mod detect;
mod fish;
mod nu;
mod prompt;
mod xonsh;
mod zsh;

pub struct EnvVars<'n> {
    vars: HashMap<&'n str, OsString>,
}

impl<'n> EnvVars<'n> {
    pub fn new() -> EnvVars<'n> {
        EnvVars { vars: HashMap::new() }
    }

    pub fn insert(&mut self, name: &'n str, value: impl Into<OsString>) {
        self.vars.insert(name, value.into());
    }

    pub fn apply(&self, cmd: &mut Command) {
        for (name, value) in &self.vars {
            cmd.env(name, value);
        }
    }
}

pub struct ShellSpawnInfo<'s, 'n> {
    settings: &'s Settings,
    env_vars: EnvVars<'n>,
    prompt: String,
}

pub fn spawn_shell(settings: &Settings, config: KubeConfig, session: &Session) -> Result<()> {
    let kind = match &settings.shell {
        Some(shell) => ShellKind::from_str(shell).ok_or_else(|| anyhow!("Invalid shell setting: {}", shell))?,
        None => detect()?,
    };

    let temp_config_file = tempfile::Builder::new()
        .prefix("kubie-config")
        .suffix(".yaml")
        .tempfile()?;
    config.write_to_file(temp_config_file.path())?;

    let temp_session_file = tempfile::Builder::new()
        .prefix("kubie-session")
        .suffix(".json")
        .tempfile()?;
    session.save(Some(temp_session_file.path()))?;

    let depth = vars::get_depth();
    let next_depth = depth + 1;

    let mut env_vars = EnvVars::new();

    // Pre-insert the KUBECONFIG variable into the shell.
    // This will make sure any shell plugins/add-ons which require this env variable
    // will have it available at the beginninng of the .rc file
    env_vars.insert("KUBECONFIG", temp_config_file.path());
    env_vars.insert("KUBIE_ACTIVE", "1");
    env_vars.insert("KUBIE_DEPTH", next_depth.to_string());
    env_vars.insert("KUBIE_KUBECONFIG", temp_config_file.path());
    env_vars.insert("KUBIE_SESSION", temp_session_file.path());
    env_vars.insert("KUBIE_STATE", state::paths::state());

    env_vars.insert("KUBIE_PROMPT_DISABLE", if settings.prompt.disable { "1" } else { "0" });
    env_vars.insert(
        "KUBIE_ZSH_USE_RPS1",
        if settings.prompt.zsh_use_rps1 { "1" } else { "0" },
    );
    env_vars.insert(
        "KUBIE_FISH_USE_RPROMPT",
        if settings.prompt.fish_use_rprompt { "1" } else { "0" },
    );
    env_vars.insert(
        "KUBIE_XONSH_USE_RIGHT_PROMPT",
        if settings.prompt.xonsh_use_right_prompt {
            "1"
        } else {
            "0"
        },
    );

    match kind {
        ShellKind::Bash => {
            env_vars.insert("KUBIE_SHELL", "bash");
        }
        ShellKind::Fish => {
            env_vars.insert("KUBIE_SHELL", "fish");
        }
        ShellKind::Xonsh => {
            env_vars.insert("KUBIE_SHELL", "xonsh");
        }
        ShellKind::Zsh => {
            env_vars.insert("KUBIE_SHELL", "zsh");
        }
        ShellKind::Nu => {
            env_vars.insert("KUBIE_SHELL", "nu");
        }
    }

    // Register temp files with the cleanup guardian so they are deleted
    // even if this process is killed abnormally.
    let config_path = temp_config_file.path().to_string_lossy().to_string();
    let session_path = temp_session_file.path().to_string_lossy().to_string();
    register_with_guardian(std::process::id(), &config_path, &session_path);

    let info = ShellSpawnInfo {
        settings,
        env_vars,
        prompt: prompt::generate_ps1(settings, next_depth, kind),
    };

    // Run start_ctx hook before spawning the shell.
    if !settings.hooks.start_ctx.is_empty() {
        run_hook(&info, &settings.hooks.start_ctx);
    }

    let result = match kind {
        ShellKind::Bash => bash::spawn_shell(&info),
        ShellKind::Fish => fish::spawn_shell(&info),
        ShellKind::Xonsh => xonsh::spawn_shell(&info),
        ShellKind::Zsh => zsh::spawn_shell(&info),
        ShellKind::Nu => nu::spawn_shell(&info),
    };

    // Run stop_ctx hook after the shell exits.
    if !settings.hooks.stop_ctx.is_empty() {
        run_hook(&info, &settings.hooks.stop_ctx);
    }

    // On normal exit, delete the temp files explicitly before drop
    // so the guardian has nothing left to clean up.
    let _ = temp_config_file.close();
    let _ = temp_session_file.close();

    result
}

/// Run a hook command in a subshell with kubie's env vars set.
fn run_hook(info: &ShellSpawnInfo, hook: &str) {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", hook]);
    info.env_vars.apply(&mut cmd);
    if let Ok(mut child) = cmd.spawn() {
        let _ = child.wait();
    }
}

/// Directory for the guardian's tracking file and pidfile.
fn guardian_dir() -> PathBuf {
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/kubie-guardian-{uid}"))
}

/// Register a kubie session's temp files with the shared cleanup guardian.
/// Uses flock on a lockfile to prevent races between concurrent kubie
/// sessions and the guardian's cleanup loop.
fn register_with_guardian(pid: u32, config_path: &str, session_path: &str) {
    let dir = guardian_dir();
    let _ = fs::create_dir_all(&dir);

    let lockfile = dir.join("lock");
    let tracking_file = dir.join("sessions");
    let pidfile = dir.join("guardian.pid");

    // Hold an exclusive lock while appending and checking the guardian.
    let Ok(lock) = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lockfile)
    else {
        return;
    };
    if lock.lock_exclusive().is_err() {
        return;
    }

    // Append this session's entry.
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&tracking_file) {
        let _ = writeln!(f, "{pid} {config_path} {session_path}");
    }

    // Check if a guardian is already running.
    let guardian_alive = fs::read_to_string(&pidfile).is_ok_and(|contents| {
        contents
            .trim()
            .parse::<i32>()
            .is_ok_and(|gpid| unsafe { libc::kill(gpid, 0) } == 0)
    });

    if !guardian_alive {
        spawn_guardian(&pidfile, &tracking_file, &lockfile);
    }

    // Lock is released on drop.
}

/// Spawn the shared guardian process. It uses flock on the same lockfile
/// to coordinate with kubie sessions appending to the tracking file.
fn spawn_guardian(pidfile: &std::path::Path, tracking_file: &std::path::Path, lockfile: &std::path::Path) {
    let pidfile_str = pidfile.to_string_lossy().to_string();
    let tracking_str = tracking_file.to_string_lossy().to_string();
    let lockfile_str = lockfile.to_string_lossy().to_string();

    // The guardian script:
    // 1. Writes its own PID to the pidfile
    // 2. Loops every 2 seconds
    // 3. Acquires flock before reading/rewriting the tracking file
    // 4. For each entry, checks if the PID is alive
    // 5. If dead, deletes the temp files
    // 6. Rewrites the tracking file with only live entries
    // 7. Exits when no entries remain
    let script = r#"
        echo $$ > "$1"
        while true; do
            sleep 2
            exec 9>"$3"
            flock 9
            if [ ! -f "$2" ]; then
                rm -f -- "$1"
                exec 9>&-
                break
            fi
            alive=""
            while IFS=' ' read -r pid cfg sess; do
                if kill -0 "$pid" 2>/dev/null; then
                    alive="${alive}${pid} ${cfg} ${sess}
"
                else
                    rm -f -- "$cfg" "$sess"
                fi
            done < "$2"
            if [ -z "$alive" ]; then
                rm -f -- "$2" "$1"
                exec 9>&-
                break
            fi
            printf '%s' "$alive" > "$2"
            exec 9>&-
        done
    "#;

    let mut cmd = Command::new("sh");
    cmd.args(["-c", script, "--", &pidfile_str, &tracking_str, &lockfile_str])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    // Put the guardian in its own session so it is not killed when the
    // terminal closes and sends SIGHUP to its process group.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }

    let _ = cmd.spawn();
}
