use anyhow::{Context, Result, anyhow};
use reqwest::blocking::{Client, RequestBuilder};
use reqwest::header::{AUTHORIZATION, HeaderValue};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Auth {
    Bearer(String),
    Basic { username: String, password: String },
}

#[derive(Debug)]
pub enum Body {
    Json(Value),
    Form(Vec<(String, String)>),
}

pub struct PinterestClient {
    client: Client,
    base_url: String,
}

impl PinterestClient {
    pub fn new(base_url: String, timeout: Option<u64>) -> Result<Self> {
        let mut builder = Client::builder().user_agent("pinterest-ads-cli/0.1.0");
        if let Some(seconds) = timeout {
            builder = builder.timeout(Duration::from_secs(seconds));
        }
        let client = builder.build().context("build http client")?;
        Ok(Self { client, base_url })
    }

    pub fn build_url(&self, path: &str) -> String {
        if path.starts_with("http://") || path.starts_with("https://") {
            return path.to_string();
        }
        let base = self.base_url.trim_end_matches('/');
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return base.to_string();
        }
        format!("{}/{}", base, path)
    }

    pub fn request(
        &self,
        method: &str,
        url: &str,
        auth: &Auth,
        query: &[(String, String)],
        body: Option<Body>,
    ) -> Result<Value> {
        let mut request = match method {
            "GET" => self.client.get(url),
            "POST" => self.client.post(url),
            "PATCH" => self.client.patch(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            other => return Err(anyhow!("unsupported method {other}")),
        };

        request = apply_auth(request, auth)?;
        if !query.is_empty() {
            request = request.query(query);
        }

        request = match (method, body) {
            ("GET" | "DELETE", Some(_)) => {
                return Err(anyhow!("request body not supported for {method}"));
            }
            (_, None) => request,
            (_, Some(Body::Json(value))) => request.json(&value),
            (_, Some(Body::Form(fields))) => request.form(&fields),
        };

        log::debug!("request {} {}", method, url);
        let resp = request.send().context("send request")?;
        let status = resp.status();
        let text = resp.text().context("read response body")?;
        if text.trim().is_empty() {
            if status.is_success() {
                return Ok(Value::Null);
            }
            return Err(anyhow!("http {}: empty response", status));
        }
        let value: Value = serde_json::from_str(&text).context("decode json")?;
        if !status.is_success() {
            return Err(anyhow!("http {}: {}", status, value));
        }
        Ok(value)
    }
}

fn apply_auth(mut req: RequestBuilder, auth: &Auth) -> Result<RequestBuilder> {
    match auth {
        Auth::Bearer(token) => {
            let value = HeaderValue::from_str(&format!("Bearer {}", token))
                .context("invalid bearer token")?;
            req = req.header(AUTHORIZATION, value);
            Ok(req)
        }
        Auth::Basic { username, password } => Ok(req.basic_auth(username, Some(password))),
    }
}
