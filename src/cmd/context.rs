use std::collections::HashSet;
use std::io::{self, IsTerminal};
use std::sync::{Arc, Mutex};

use anyhow::Result;

use crate::cloud::{self, CloudContext};
use crate::cmd::SelectResult;
use crate::kubeconfig::{self, Installed};
use crate::kubectl;
use crate::session::Session;
use crate::settings::Settings;
use crate::shell::spawn_shell;
use crate::state::State;
use crate::vars;

fn enter_context(
    settings: &Settings,
    installed: Installed,
    context_name: &str,
    namespace_name: Option<&str>,
    recursive: bool,
) -> Result<()> {
    let state = State::load()?;
    let mut session = Session::load()?;

    let kubeconfig = if context_name == "-" {
        if let Some(previous) = session.get_last_context() {
            // Inside a kubie shell: switch to the previous context from session history
            let ns = namespace_name.or(previous.namespace.as_deref());
            installed.make_kubeconfig_for_context(&previous.context, ns)?
        } else if let Some(ref last) = state.last_context {
            // Outside a kubie shell: fall back to the last globally-used context
            let ns = namespace_name.or_else(|| state.namespace_history.get(last).and_then(|s| s.as_deref()));
            installed.make_kubeconfig_for_context(last, ns)?
        } else {
            anyhow::bail!("There is no previous context to switch to.");
        }
    } else {
        let ns = namespace_name.or_else(|| state.namespace_history.get(context_name).and_then(|s| s.as_deref()));
        installed.make_kubeconfig_for_context(context_name, ns)?
    };

    session.record_context_entry(
        &kubeconfig.contexts[0].name,
        kubeconfig.contexts[0].context.namespace.as_deref(),
    )?;

    if settings.behavior.validate_namespaces.can_list_namespaces() {
        if let Some(namespace_name) = namespace_name {
            let namespaces = kubectl::get_namespaces(Some(&kubeconfig))?;
            if !namespaces.iter().any(|x| x == namespace_name) {
                eprintln!("Warning: namespace {namespace_name} does not exist.");
            }
        }
    }

    if vars::is_kubie_active() && !recursive {
        let path = kubeconfig::get_kubeconfig_path()?;
        kubeconfig.write_to_file(path.as_path())?;
        session.save(None)?;
    } else {
        spawn_shell(settings, kubeconfig, &session)?;
    }

    Ok(())
}

/// Enter a cloud context by downloading its kubeconfig on-demand,
/// parsing it in memory, and feeding it into the standard context entry flow.
fn enter_cloud_context(
    settings: &Settings,
    cloud_ctx: &CloudContext,
    namespace_name: Option<&str>,
    recursive: bool,
) -> Result<()> {
    let yaml = cloud::download_kubeconfig(settings, cloud_ctx)?;
    let installed = kubeconfig::parse_kubeconfig_from_str(&yaml)?;
    enter_context(settings, installed, &cloud_ctx.context_name, namespace_name, recursive)
}

/// Interactive selection that merges local and cloud contexts.
/// Cached cloud contexts appear instantly alongside local ones. A background
/// thread discovers fresh cloud contexts and streams new names into skim live.
fn select_or_list_merged(
    settings: &Settings,
    installed: &Installed,
    cloud_contexts: &Arc<Mutex<Vec<CloudContext>>>,
) -> Result<SelectResult> {
    use crate::skim::TaggedItem;

    let local_names: HashSet<String> = installed.contexts.iter().map(|c| c.item.name.clone()).collect();

    // Build tagged items: local contexts have no tag, cloud-only contexts
    // are tagged with their provider name (e.g. "[doctl]").
    let mut items: Vec<TaggedItem> = installed
        .contexts
        .iter()
        .map(|c| TaggedItem::new(c.item.name.clone(), None))
        .collect();

    // Add cached cloud contexts that aren't already local.
    {
        let cloud = cloud_contexts.lock().unwrap_or_else(|e| e.into_inner());
        for cc in cloud.iter() {
            if !local_names.contains(&cc.context_name) {
                items.push(TaggedItem::new(cc.context_name.clone(), Some(cc.provider.clone())));
            }
        }
    }

    items.sort_by(|a, b| a.name.cmp(&b.name));
    items.dedup_by(|a, b| a.name == b.name);

    if items.is_empty() {
        anyhow::bail!("No contexts found");
    }
    if items.len() == 1 {
        return Ok(SelectResult::Selected(items[0].name.clone()));
    }

    if io::stdout().is_terminal() {
        let has_cloud_providers = !cloud::enabled_providers(&settings.cloud).is_empty();

        if has_cloud_providers {
            let known_names: HashSet<String> = items.iter().map(|i| i.name.clone()).collect();
            let cloud_for_bg = Arc::clone(cloud_contexts);
            let settings_clone = settings.cloud.clone();

            // NOTE: skim shows the list in reverse order
            items.reverse();

            let selected = crate::skim::select_with_bg(&settings.fzf, items, move |tx| {
                let fresh = discover_and_cache(&settings_clone);

                // Update the shared cloud context list with fresh data.
                {
                    let mut cloud = cloud_for_bg.lock().unwrap_or_else(|e| e.into_inner());
                    *cloud = fresh.clone();
                }

                // Push only new names into skim, tagged with their provider.
                let new: Vec<TaggedItem> = fresh
                    .iter()
                    .filter(|c| !known_names.contains(&c.context_name))
                    .map(|c| TaggedItem::new(c.context_name.clone(), Some(c.provider.clone())))
                    .collect();

                if !new.is_empty() {
                    let skim_items: Vec<Arc<dyn skim::SkimItem>> = new
                        .into_iter()
                        .map(|item| Arc::new(item) as Arc<dyn skim::SkimItem>)
                        .collect();
                    let _ = tx.send(skim_items);
                }
            })?;

            match selected {
                Some(name) => Ok(SelectResult::Selected(name)),
                None => Ok(SelectResult::Cancelled),
            }
        } else {
            // No cloud providers -- use the simple picker.
            let names: Vec<String> = items.into_iter().rev().map(|i| i.name).collect();
            match crate::skim::select(&settings.fzf, names)? {
                Some(name) => Ok(SelectResult::Selected(name)),
                None => Ok(SelectResult::Cancelled),
            }
        }
    } else {
        for item in items {
            println!("{}", item.name);
        }
        Ok(SelectResult::Listed)
    }
}

/// Run cloud discovery and update the cache (callable from a background thread).
fn discover_and_cache(cloud_settings: &crate::settings::CloudSettings) -> Vec<CloudContext> {
    let mut contexts = Vec::new();
    for provider in cloud::enabled_providers(cloud_settings) {
        match provider.discover() {
            Ok(discovered) => {
                let _ = cloud::cache::save_contexts(provider.name(), &discovered);
                contexts.extend(discovered);
            }
            Err(e) => {
                eprintln!("Warning: cloud provider '{}' discovery failed: {e}", provider.name());
            }
        }
    }
    contexts
}

pub fn context(
    settings: &Settings,
    context_name: Option<String>,
    namespace_name: Option<String>,
    kubeconfigs: Vec<String>,
    recursive: bool,
) -> Result<()> {
    let has_cloud = !cloud::enabled_providers(&settings.cloud).is_empty();

    let installed = if kubeconfigs.is_empty() {
        if has_cloud {
            // Don't bail on empty local contexts when cloud providers may supply them.
            kubeconfig::load_installed_contexts(settings)?
        } else {
            kubeconfig::get_installed_contexts(settings)?
        }
    } else {
        kubeconfig::get_kubeconfigs_contexts(&kubeconfigs)?
    };

    // Load cached cloud contexts instantly (no network calls).
    let cloud_contexts = if kubeconfigs.is_empty() {
        Arc::new(Mutex::new(cloud::load_cached(settings)))
    } else {
        Arc::new(Mutex::new(Vec::new()))
    };

    let local_names: HashSet<String> = installed.contexts.iter().map(|c| c.item.name.clone()).collect();

    let context_name = match context_name {
        Some(name) => name,
        None => match select_or_list_merged(settings, &installed, &cloud_contexts)? {
            SelectResult::Selected(x) => x,
            _ => return Ok(()),
        },
    };

    // Check if this is a cloud context (check the potentially-updated shared list).
    let cloud_ctx = {
        let cloud = cloud_contexts.lock().unwrap_or_else(|e| e.into_inner());
        cloud.iter().find(|c| c.context_name == context_name).cloned()
    };

    if let Some(ref cc) = cloud_ctx {
        if local_names.contains(&context_name) {
            // Context exists locally -- prefer the local version.
            enter_context(settings, installed, &context_name, namespace_name.as_deref(), recursive)
        } else {
            enter_cloud_context(settings, cc, namespace_name.as_deref(), recursive)
        }
    } else {
        enter_context(settings, installed, &context_name, namespace_name.as_deref(), recursive)
    }
}
