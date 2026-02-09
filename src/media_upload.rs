use anyhow::{Context, Result, anyhow};
use reqwest::blocking::{Client, multipart};
use serde_json::Value;
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::client::{Auth, Body, PinterestClient};
use crate::sources::SourceFile;

pub fn upload_media(
    api: &PinterestClient,
    auth: &Auth,
    media_type: &str,
    file: &SourceFile,
    wait: bool,
) -> Result<Value> {
    let register_url = api.build_url("/media");
    let register = api.request(
        "POST",
        &register_url,
        auth,
        &[],
        Some(Body::Json(serde_json::json!({ "media_type": media_type }))),
    )?;

    let media_id = register
        .get("media_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing media_id"))?
        .to_string();
    let upload_url = register
        .get("upload_url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("missing upload_url"))?
        .to_string();
    let params = register
        .get("upload_parameters")
        .and_then(|v| v.as_object())
        .ok_or_else(|| anyhow!("missing upload_parameters"))?;

    upload_to_s3(&upload_url, params, file)?;

    if !wait {
        return Ok(register);
    }

    wait_for_processing(api, auth, &media_id, Duration::from_secs(180))
}

fn upload_to_s3(
    upload_url: &str,
    params: &serde_json::Map<String, Value>,
    file: &SourceFile,
) -> Result<()> {
    let http = Client::builder()
        .user_agent("pinterest-ads-cli/0.1.0")
        .build()
        .context("build upload client")?;

    let mut form = multipart::Form::new();
    for (k, v) in params {
        let Some(s) = v.as_str() else { continue };
        form = form.text(k.clone(), s.to_string());
    }

    // S3 form uploads conventionally use "file" as the part name.
    let part = multipart::Part::file(&file.path)
        .with_context(|| format!("open file {}", file.path.display()))?
        .file_name(file.file_name.clone());
    form = form.part("file", part);

    let resp = http
        .post(upload_url)
        .multipart(form)
        .send()
        .context("upload media")?;
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let text = resp.text().unwrap_or_default();
    Err(anyhow!("upload failed (http {}): {}", status, text))
}

fn wait_for_processing(
    api: &PinterestClient,
    auth: &Auth,
    media_id: &str,
    timeout: Duration,
) -> Result<Value> {
    let start = Instant::now();
    loop {
        let url = api.build_url(&format!("/media/{}", media_id));
        let resp = api.request("GET", &url, auth, &[], None)?;
        let status = resp
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        match status {
            "succeeded" => return Ok(resp),
            "failed" => return Err(anyhow!("media status: failed")),
            "registered" | "processing" => {}
            other => return Err(anyhow!("media status: {other}")),
        }

        if start.elapsed() >= timeout {
            return Err(anyhow!("media processing timeout"));
        }
        sleep(Duration::from_secs(2));
    }
}
