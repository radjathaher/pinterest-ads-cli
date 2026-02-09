use anyhow::{Result, anyhow};
use serde_json::Value;

use crate::client::{Auth, PinterestClient};

pub fn paginate_all(
    client: &PinterestClient,
    method: &str,
    url: &str,
    auth: &Auth,
    query: &[(String, String)],
    max_pages: u64,
    max_items: u64,
) -> Result<Value> {
    if method != "GET" {
        return Err(anyhow!("--all only supported for GET"));
    }

    let mut base_query: Vec<(String, String)> = Vec::new();
    let mut bookmark: Option<String> = None;
    for (k, v) in query {
        if k == "bookmark" {
            bookmark = Some(v.clone());
        } else {
            base_query.push((k.clone(), v.clone()));
        }
    }

    let mut pages = 0u64;
    let mut items: Vec<Value> = Vec::new();

    loop {
        pages += 1;
        if max_pages > 0 && pages > max_pages {
            break;
        }

        let mut q = base_query.clone();
        if let Some(b) = &bookmark {
            q.push(("bookmark".to_string(), b.clone()));
        }

        let resp = client.request("GET", url, auth, &q, None)?;
        let data = resp
            .get("items")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("expected paginated response with items[]"))?;

        for item in data {
            items.push(item.clone());
            if max_items > 0 && items.len() as u64 >= max_items {
                return Ok(serde_json::json!({ "items": items }));
            }
        }

        bookmark = resp
            .get("bookmark")
            .and_then(|v| v.as_str())
            .map(|v| v.to_string())
            .filter(|v| !v.is_empty());

        if bookmark.is_none() {
            break;
        }
    }

    Ok(serde_json::json!({ "items": items }))
}
