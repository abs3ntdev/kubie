use std::process::{Command, Stdio};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::{CloudContext, CloudProvider};

/// A doctl auth context from `doctl auth list`.
#[derive(Debug, Deserialize)]
struct AuthContext {
    name: String,
}

/// Raw cluster data as returned by `doctl kubernetes cluster list`.
/// Only the fields we need are deserialized; unknown fields are ignored by serde.
#[derive(Debug, Deserialize)]
struct DoctlCluster {
    id: String,
    name: String,
    region: String,
}

/// Cluster metadata used during discovery.
#[derive(Debug, Clone, Deserialize)]
struct ClusterInfo {
    id: String,
    name: String,
    region: String,
    doctl_context: String,
}

impl ClusterInfo {
    /// The kubernetes context name as doctl generates it: `do-<region>-<cluster-name>`.
    pub fn kube_context_name(&self) -> String {
        format!("do-{}-{}", self.region, self.name)
    }

    /// The opaque provider key: `<doctl_context>:<cluster_id>`.
    pub fn provider_key(&self) -> String {
        format!("{}:{}", self.doctl_context, self.id)
    }
}

pub struct DoctlProvider {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl CloudProvider for DoctlProvider {
    fn name(&self) -> &'static str {
        "doctl"
    }

    fn discover(&self) -> Result<Vec<CloudContext>> {
        let mut auth_contexts = list_auth_contexts()?;

        if !self.include.is_empty() {
            auth_contexts.retain(|c| self.include.iter().any(|i| i == &c.name));
        }
        auth_contexts.retain(|c| !self.exclude.iter().any(|e| e == &c.name));

        let cloud_contexts: Mutex<Vec<CloudContext>> = Mutex::new(Vec::new());

        std::thread::scope(|s| {
            for ctx in &auth_contexts {
                let cloud_contexts = &cloud_contexts;
                s.spawn(move || {
                    if let Ok(clusters) = list_clusters(&ctx.name) {
                        let mut ctxs = cloud_contexts.lock().unwrap_or_else(|e| e.into_inner());
                        for c in clusters {
                            ctxs.push(CloudContext {
                                context_name: c.kube_context_name(),
                                provider_key: c.provider_key(),
                                provider: "doctl".into(),
                            });
                        }
                    }
                });
            }
        });

        Ok(cloud_contexts.into_inner().unwrap_or_default())
    }

    fn download_kubeconfig(&self, provider_key: &str) -> Result<String> {
        let (doctl_context, cluster_id) = provider_key
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("Invalid doctl provider key: {provider_key}"))?;
        download_kubeconfig_raw(doctl_context, cluster_id)
    }
}

/// Get all doctl auth contexts, excluding the "default" placeholder.
fn list_auth_contexts() -> Result<Vec<AuthContext>> {
    let output = Command::new("doctl")
        .args(["auth", "list", "-o", "json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run doctl auth list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("doctl auth list failed: {stderr}");
    }

    let contexts: Vec<AuthContext> =
        serde_json::from_slice(&output.stdout).context("Failed to parse doctl auth list output")?;

    Ok(contexts.into_iter().filter(|c| c.name != "default").collect())
}

/// List kubernetes clusters for a doctl auth context.
/// Uses the `--context` flag so no global auth state is mutated.
fn list_clusters(doctl_context: &str) -> Result<Vec<ClusterInfo>> {
    let output = Command::new("doctl")
        .args([
            "kubernetes",
            "cluster",
            "list",
            "--context",
            doctl_context,
            "-o",
            "json",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to list kubernetes clusters")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("doctl kubernetes cluster list failed: {stderr}");
    }

    let clusters: Vec<DoctlCluster> =
        serde_json::from_slice(&output.stdout).context("Failed to parse doctl kubernetes cluster list output")?;

    Ok(clusters
        .into_iter()
        .map(|c| ClusterInfo {
            id: c.id,
            name: c.name,
            region: c.region,
            doctl_context: doctl_context.to_string(),
        })
        .collect())
}

/// Download the kubeconfig for a specific cluster.
/// Uses the `--context` flag so no global auth state is mutated.
fn download_kubeconfig_raw(doctl_context: &str, cluster_id: &str) -> Result<String> {
    let output = Command::new("doctl")
        .args([
            "kubernetes",
            "cluster",
            "kubeconfig",
            "show",
            cluster_id,
            "--context",
            doctl_context,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to download kubeconfig")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to download kubeconfig for cluster {cluster_id}: {stderr}");
    }

    String::from_utf8(output.stdout).context("Kubeconfig output is not valid UTF-8")
}
