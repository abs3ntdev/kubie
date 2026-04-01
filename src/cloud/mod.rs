pub mod cache;
pub mod doctl;
pub mod gcloud;

use anyhow::Result;

use crate::settings::Settings;

/// A discovered cloud cluster. Each provider populates these fields
/// so that the rest of kubie can treat all cloud clusters uniformly.
#[derive(Debug, Clone)]
pub struct CloudContext {
    /// The kubernetes context name (e.g. `do-sfo3-my-cluster`).
    pub context_name: String,
    /// An opaque key that the provider uses to download the kubeconfig.
    /// For doctl this is `doctl_context:cluster_id`.
    pub provider_key: String,
    /// Which provider owns this context (e.g. `"doctl"`).
    pub provider: String,
}

/// Trait that each cloud provider implements. Kept intentionally minimal
/// so that adding a new provider (e.g. gcloud, aws) requires only two methods.
pub trait CloudProvider: Send {
    /// A short identifier used as the cache subdirectory name (e.g. `"doctl"`).
    fn name(&self) -> &'static str;

    /// Discover all available cluster context names.
    /// Returns a list of `CloudContext` entries for the picker.
    fn discover(&self) -> Result<Vec<CloudContext>>;

    /// Download the kubeconfig YAML for the given context.
    /// `provider_key` is the opaque key from `CloudContext::provider_key`.
    fn download_kubeconfig(&self, provider_key: &str) -> Result<String>;
}

/// Build the list of enabled cloud providers from cloud settings.
pub fn enabled_providers(cloud: &crate::settings::CloudSettings) -> Vec<Box<dyn CloudProvider>> {
    let mut providers: Vec<Box<dyn CloudProvider>> = Vec::new();

    if cloud.doctl.enabled {
        providers.push(Box::new(doctl::DoctlProvider {
            include: cloud.doctl.include.clone(),
            exclude: cloud.doctl.exclude.clone(),
        }));
    }

    if cloud.gcloud.enabled {
        providers.push(Box::new(gcloud::GcloudProvider {
            include: cloud.gcloud.include.clone(),
            exclude: cloud.gcloud.exclude.clone(),
        }));
    }

    providers
}

/// Load cached cloud contexts instantly (no network calls).
/// Returns an empty vec if no providers are enabled or no cache exists.
pub fn load_cached(settings: &Settings) -> Vec<CloudContext> {
    let mut contexts = Vec::new();
    for provider in enabled_providers(&settings.cloud) {
        if let Ok(Some(cached)) = cache::load_contexts(provider.name()) {
            contexts.extend(cached);
        }
    }
    contexts
}

/// Download the kubeconfig for a cloud context by finding its provider.
pub fn download_kubeconfig(settings: &Settings, cloud_ctx: &CloudContext) -> Result<String> {
    for provider in enabled_providers(&settings.cloud) {
        if provider.name() == cloud_ctx.provider {
            return provider.download_kubeconfig(&cloud_ctx.provider_key);
        }
    }
    anyhow::bail!("No cloud provider '{}' is enabled", cloud_ctx.provider)
}
