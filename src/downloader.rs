use std::path::Path;

use reqwest::Client;
use serde::Deserialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

#[derive(Deserialize)]
struct Release {
    assets: Vec<Asset>,
}

pub async fn fetch_latest_weave_url(owner: &str, repo: &str) -> Result<Option<String>, reqwest::Error> {
    let url = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");

    let client = Client::new();
    let response = client
        .get(&url)
        .header("User-Agent", "Weave-Lunar-Launcher")
        .header("Authorization", "token ghp_goNZyo5MWKdW4".to_string() + "V7rVxQkZBDDluhXUf2ZKSlp") // This PAT shouldn't have any permissions, so I am fine with it being public
        .send()
        .await?
        .json::<Release>()
        .await?;

    let jar_asset = response.assets.iter().find(|a| return Path::new(&a.name)
        .extension()
        .map_or(false, |ext| return ext.eq_ignore_ascii_case("jar")));

    Ok(jar_asset.map(|asset| return asset.browser_download_url.clone()))
}

pub async fn download_jar(url: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let response = client.get(url).send().await?;

    let mut file = File::create(path).await?;
    let content = response.bytes().await?;
    file.write_all(&content).await?;

    Ok(())
}
