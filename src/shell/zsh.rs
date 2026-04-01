use std::fs::File;
use std::io::{BufWriter, Write};
use std::process::Command;

use anyhow::{Context, Result};
use tempfile::tempdir;

use super::ShellSpawnInfo;

pub fn spawn_shell(info: &ShellSpawnInfo) -> Result<()> {
    let dir = tempdir()?;
    {
        let zshrc_path = dir.path().join(".zshrc");
        let zshrc = File::create(zshrc_path).context("Could not open zshrc file")?;
        let mut zshrc_buf = BufWriter::new(zshrc);
        write!(
            zshrc_buf,
            r#"
# Save the original ZDOTDIR before kubie overwrites it.
# If the user has a custom ZDOTDIR (set in their environment or .zshenv),
# we need to preserve it so plugins and configs that reference $ZDOTDIR
# continue to work after initialization.
_KUBIE_ORIG_ZDOTDIR="${{ZDOTDIR:-$HOME}}"

# Source system zshenv.
if [[ -f "/etc/zshenv" ]] ; then
    source "/etc/zshenv"
elif [[ -f "/etc/zsh/zshenv" ]] ; then
    source "/etc/zsh/zshenv"
fi

# Source user zshenv. If it changes ZDOTDIR, capture the new value.
if [[ -f "$_KUBIE_ORIG_ZDOTDIR/.zshenv" ]] ; then
    _KUBIE_PRE_ZDOTDIR="$ZDOTDIR"
    source "$_KUBIE_ORIG_ZDOTDIR/.zshenv"
    if [[ "$ZDOTDIR" != "$_KUBIE_PRE_ZDOTDIR" ]]; then
        _KUBIE_ORIG_ZDOTDIR="$ZDOTDIR"
    fi
    unset _KUBIE_PRE_ZDOTDIR
fi

# Explicitly set HISTFILE so history is preserved across kubie sessions
# instead of being written to the temp ZDOTDIR and lost.
export HISTFILE="${{HISTFILE:-$_KUBIE_ORIG_ZDOTDIR/.zsh_history}}"

# Source system and user zprofile.
if [[ -f "/etc/zprofile" ]] ; then
    source "/etc/zprofile"
elif [[ -f "/etc/zsh/zprofile" ]] ; then
    source "/etc/zsh/zprofile"
fi

if [[ -f "$_KUBIE_ORIG_ZDOTDIR/.zprofile" ]] ; then
    source "$_KUBIE_ORIG_ZDOTDIR/.zprofile"
fi

# Source system and user zshrc.
if [[ -f "/etc/zshrc" ]] ; then
    source "/etc/zshrc"
elif [[ -f "/etc/zsh/zshrc" ]] ; then
    source "/etc/zsh/zshrc"
fi

if [[ -f "$_KUBIE_ORIG_ZDOTDIR/.zshrc" ]] ; then
    ZDOTDIR="$_KUBIE_ORIG_ZDOTDIR" source "$_KUBIE_ORIG_ZDOTDIR/.zshrc"
fi

# Source system and user zlogin.
if [[ -f "/etc/zlogin" ]] ; then
    source "/etc/zlogin"
elif [[ -f "/etc/zsh/zlogin" ]] ; then
    source "/etc/zsh/zlogin"
fi

if [[ -f "$_KUBIE_ORIG_ZDOTDIR/.zlogin" ]] ; then
    source "$_KUBIE_ORIG_ZDOTDIR/.zlogin"
fi

# Restore ZDOTDIR to the user's original value so plugins and scripts
# that reference $ZDOTDIR at runtime point to the right place.
ZDOTDIR="$_KUBIE_ORIG_ZDOTDIR"
unset _KUBIE_ORIG_ZDOTDIR

autoload -Uz add-zsh-hook

# Ensure KUBECONFIG is always set before a command runs,
# in case something overwrote it.
function __kubie_cmd_pre_exec__() {{
    export KUBECONFIG="$KUBIE_KUBECONFIG"
}}

add-zsh-hook preexec __kubie_cmd_pre_exec__
"#,
        )?;

        if !info.settings.prompt.disable {
            write!(
                zshrc_buf,
                r#"
setopt PROMPT_SUBST

function __kubie_cmd_pre_cmd__() {{
    local KUBIE_PROMPT=$'{}'

    if [[ "$KUBIE_ZSH_USE_RPS1" == "1" ]] ; then
        if [[ "$RPS1" != *"$KUBIE_PROMPT"* ]] ; then
            if [[ -z "$RPS1" ]] ; then
                RPS1="$KUBIE_PROMPT"
            else
                RPS1="$KUBIE_PROMPT $RPS1"
            fi
        fi
    else
        if [[ "$PS1" != *"$KUBIE_PROMPT"* ]] ; then
            PS1="$KUBIE_PROMPT $PS1"
        fi
    fi
}}

add-zsh-hook precmd __kubie_cmd_pre_cmd__
"#,
                info.prompt
            )?;
        }
    }

    let mut cmd = Command::new("zsh");
    cmd.env("ZDOTDIR", dir.path());
    info.env_vars.apply(&mut cmd);

    let mut child = cmd.spawn()?;
    child.wait()?;

    Ok(())
}
