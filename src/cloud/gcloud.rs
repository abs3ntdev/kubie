use std::process::{Command, Stdio};
use std::sync::Mutex;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use super::{CloudContext, CloudProvider};

#[derive(Debug, Deserialize)]
struct GcloudProject {
    #[serde(rename = "projectId")]
    project_id: String,
}

/// Only the fields we need; unknown fields are ignored by serde.
#[derive(Debug, Deserialize)]
struct GkeCluster {
    name: String,
    location: String,
}

struct ClusterRef {
    project: String,
    location: String,
    name: String,
}

impl ClusterRef {
    /// The kubernetes context name as gcloud generates it: `gke_<project>_<location>_<cluster>`.
    fn kube_context_name(&self) -> String {
        format!("gke_{}_{}_{}", self.project, self.location, self.name)
    }

    fn provider_key(&self) -> String {
        format!("{}:{}:{}", self.project, self.location, self.name)
    }
}

pub struct GcloudProvider {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
}

impl CloudProvider for GcloudProvider {
    fn name(&self) -> &'static str {
        "gcloud"
    }

    fn discover(&self) -> Result<Vec<CloudContext>> {
        let mut projects = list_projects()?;

        if !self.include.is_empty() {
            projects.retain(|p| self.include.iter().any(|i| i == &p.project_id));
        }
        projects.retain(|p| !self.exclude.iter().any(|e| e == &p.project_id));

        let cloud_contexts: Mutex<Vec<CloudContext>> = Mutex::new(Vec::new());

        std::thread::scope(|s| {
            for project in &projects {
                let cloud_contexts = &cloud_contexts;
                s.spawn(move || {
                    if let Ok(clusters) = list_clusters(&project.project_id) {
                        let mut ctxs = cloud_contexts.lock().unwrap_or_else(|e| e.into_inner());
                        for c in clusters {
                            let cr = ClusterRef {
                                project: project.project_id.clone(),
                                location: c.location,
                                name: c.name,
                            };
                            ctxs.push(CloudContext {
                                context_name: cr.kube_context_name(),
                                provider_key: cr.provider_key(),
                                provider: "gcloud".into(),
                            });
                        }
                    }
                });
            }
        });

        Ok(cloud_contexts.into_inner().unwrap_or_default())
    }

    fn download_kubeconfig(&self, provider_key: &str) -> Result<String> {
        let parts: Vec<&str> = provider_key.splitn(3, ':').collect();
        if parts.len() != 3 {
            bail!("Invalid gcloud provider key: {provider_key}");
        }
        download_kubeconfig_raw(parts[0], parts[1], parts[2])
    }
}

fn list_projects() -> Result<Vec<GcloudProject>> {
    let output = Command::new("gcloud")
        .args(["projects", "list", "--format=json(projectId)"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run gcloud projects list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("gcloud projects list failed: {stderr}");
    }

    serde_json::from_slice(&output.stdout).context("Failed to parse gcloud projects list output")
}

fn list_clusters(project: &str) -> Result<Vec<GkeCluster>> {
    let output = Command::new("gcloud")
        .args([
            "container",
            "clusters",
            "list",
            "--project",
            project,
            "--format=json(name,location)",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to list GKE clusters")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("gcloud container clusters list failed: {stderr}");
    }

    serde_json::from_slice(&output.stdout).context("Failed to parse gcloud clusters list output")
}

/// Download a kubeconfig for a specific GKE cluster.
/// gcloud doesn't have a `show` command -- `get-credentials` writes to a file.
/// We use a temporary KUBECONFIG, read it back, then delete it.
fn download_kubeconfig_raw(project: &str, location: &str, cluster: &str) -> Result<String> {
    let tmp = tempfile::NamedTempFile::new().context("Failed to create temp file for kubeconfig")?;
    let tmp_path = tmp.path().to_string_lossy().to_string();

    let output = Command::new("gcloud")
        .args([
            "container",
            "clusters",
            "get-credentials",
            cluster,
            "--project",
            project,
            "--location",
            location,
            "--quiet",
        ])
        .env("KUBECONFIG", &tmp_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to run gcloud get-credentials")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to get credentials for cluster {cluster}: {stderr}");
    }

    std::fs::read_to_string(tmp.path()).context("Failed to read kubeconfig from temp file")
}
