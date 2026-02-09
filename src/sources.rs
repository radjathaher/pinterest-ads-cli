use anyhow::{Context, Result, anyhow};
use reqwest::blocking::Client;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

use crate::s3;

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub file_name: String,
    _temp: Option<tempfile::TempPath>,
}

pub fn looks_like_source(value: &str) -> bool {
    value.starts_with('@')
        || value.starts_with("file://")
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("s3://")
        || Path::new(value).exists()
}

pub fn resolve_source(value: &str) -> Result<SourceFile> {
    if value.starts_with("s3://") {
        return download_s3(value);
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return download_http(value);
    }

    let local = local_path(value);
    if local.exists() {
        let file_name = local
            .file_name()
            .and_then(|v| v.to_str())
            .unwrap_or("input")
            .to_string();
        return Ok(SourceFile {
            path: local,
            file_name,
            _temp: None,
        });
    }

    Err(anyhow!("file not found: {value}"))
}

pub fn read_source_to_string(value: &str) -> Result<String> {
    let file = resolve_source(value)?;
    let mut f = File::open(&file.path).with_context(|| format!("open {}", file.path.display()))?;
    let mut buf = String::new();
    f.read_to_string(&mut buf).context("read source")?;
    Ok(buf)
}

fn download_http(url: &str) -> Result<SourceFile> {
    let client = Client::new();
    let mut resp = client.get(url).send().context("download url")?;
    let mut file = NamedTempFile::new().context("create temp file")?;
    resp.copy_to(&mut file).context("write temp file")?;
    let temp_path = file.into_temp_path();
    let path = temp_path.to_path_buf();
    let file_name = url
        .split('/')
        .last()
        .filter(|v| !v.is_empty())
        .unwrap_or("download")
        .to_string();
    Ok(SourceFile {
        path,
        file_name,
        _temp: Some(temp_path),
    })
}

fn download_s3(url: &str) -> Result<SourceFile> {
    let (bucket, key) = s3::parse_s3_url(url)?;
    let mut file = NamedTempFile::new().context("create temp file")?;
    s3::download_object_blocking(&bucket, &key, &mut file)?;
    let temp_path = file.into_temp_path();
    let path = temp_path.to_path_buf();
    let file_name = Path::new(&key)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("s3-object")
        .to_string();
    Ok(SourceFile {
        path,
        file_name,
        _temp: Some(temp_path),
    })
}

fn local_path(value: &str) -> PathBuf {
    if let Some(path) = value.strip_prefix('@') {
        return PathBuf::from(path);
    }
    if let Some(path) = value.strip_prefix("file://") {
        return PathBuf::from(path);
    }
    PathBuf::from(value)
}
