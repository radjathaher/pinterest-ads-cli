mod client;
mod command_tree;
mod media_upload;
mod pagination;
mod s3;
mod sources;

use anyhow::{Context, Result, anyhow};
use clap::{Arg, ArgAction, Command};
use command_tree::{CommandTree, Operation, ParamDef};
use serde_json::Value;
use std::env;
use std::io::Write;

use crate::client::{Auth, Body, PinterestClient};

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let tree = command_tree::load_command_tree();
    let cli = build_cli(&tree);
    let matches = cli.get_matches();

    if let Some(matches) = matches.subcommand_matches("list") {
        return handle_list(&tree, matches);
    }
    if let Some(matches) = matches.subcommand_matches("describe") {
        return handle_describe(&tree, matches);
    }
    if let Some(matches) = matches.subcommand_matches("tree") {
        return handle_tree(&tree, matches);
    }
    if let Some(matches) = matches.subcommand_matches("raw") {
        return handle_raw(&tree, &matches);
    }

    let config = load_config(&tree, &matches)?;
    setup_logging(matches.get_flag("debug"))?;

    let client = PinterestClient::new(config.base_url.clone(), config.timeout)?;

    let pretty = matches.get_flag("pretty");
    let raw_output = matches.get_flag("raw_output");
    let all = matches.get_flag("all");
    let max_pages = matches.get_one::<u64>("max_pages").copied().unwrap_or(0);
    let max_items = matches.get_one::<u64>("max_items").copied().unwrap_or(0);

    let (res_name, res_matches) = matches
        .subcommand()
        .ok_or_else(|| anyhow!("resource required"))?;
    let (op_name, op_matches) = res_matches
        .subcommand()
        .ok_or_else(|| anyhow!("operation required"))?;

    if res_name == "media" && op_name == "upload" {
        return handle_media_upload(&client, &config, op_matches, pretty);
    }

    let op = find_op(&tree, res_name, op_name)
        .ok_or_else(|| anyhow!("unknown command {res_name} {op_name}"))?;

    let auth = select_auth(op, &config)?;
    let path = build_path(op, op_matches, &config)?;
    let url = client.build_url(&path);

    let query = build_query_params(op, op_matches)?;
    let body = build_body(op, op_matches)?;

    let response = if all && op.paginated {
        pagination::paginate_all(
            &client,
            op.method.as_str(),
            &url,
            &auth,
            &query,
            max_pages,
            max_items,
        )?
    } else {
        client.request(op.method.as_str(), &url, &auth, &query, body)?
    };

    let output = if raw_output {
        response
    } else if let Some(items) = response.get("items") {
        items.clone()
    } else {
        response
    };

    write_json(&output, pretty)?;
    Ok(())
}

struct Config {
    base_url: String,
    access_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    conversion_token: Option<String>,
    ad_account_id: Option<String>,
    timeout: Option<u64>,
}

fn load_config(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<Config> {
    let base_url = matches
        .get_one::<String>("base_url")
        .cloned()
        .or_else(|| env::var("PINTEREST_BASE_URL").ok())
        .unwrap_or_else(|| tree.base_url.clone());

    let access_token = matches
        .get_one::<String>("access_token")
        .cloned()
        .or_else(|| env::var("PINTEREST_ACCESS_TOKEN").ok());

    let client_id = matches
        .get_one::<String>("client_id")
        .cloned()
        .or_else(|| env::var("PINTEREST_CLIENT_ID").ok());

    let client_secret = matches
        .get_one::<String>("client_secret")
        .cloned()
        .or_else(|| env::var("PINTEREST_CLIENT_SECRET").ok());

    let conversion_token = matches
        .get_one::<String>("conversion_token")
        .cloned()
        .or_else(|| env::var("PINTEREST_CONVERSION_TOKEN").ok());

    let ad_account_id = matches
        .get_one::<String>("ad_account_id")
        .cloned()
        .or_else(|| env::var("PINTEREST_AD_ACCOUNT_ID").ok());

    let timeout = matches.get_one::<u64>("timeout").copied();

    Ok(Config {
        base_url,
        access_token,
        client_id,
        client_secret,
        conversion_token,
        ad_account_id,
        timeout,
    })
}

fn setup_logging(debug: bool) -> Result<()> {
    if debug {
        env_logger::Builder::from_env("RUST_LOG")
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_env("RUST_LOG")
            .filter_level(log::LevelFilter::Warn)
            .init();
    }
    Ok(())
}

fn build_cli(tree: &CommandTree) -> Command {
    let mut cmd = Command::new("pinterest-ads")
        .about("Pinterest Ads API CLI (auto-generated from OpenAPI)")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .arg(
            Arg::new("access_token")
                .long("access-token")
                .global(true)
                .value_name("TOKEN")
                .help("Bearer access token (env: PINTEREST_ACCESS_TOKEN)"),
        )
        .arg(
            Arg::new("client_id")
                .long("client-id")
                .global(true)
                .value_name("ID")
                .help("OAuth client id / app id (env: PINTEREST_CLIENT_ID)"),
        )
        .arg(
            Arg::new("client_secret")
                .long("client-secret")
                .global(true)
                .value_name("SECRET")
                .help("OAuth client secret (env: PINTEREST_CLIENT_SECRET)"),
        )
        .arg(
            Arg::new("conversion_token")
                .long("conversion-token")
                .global(true)
                .value_name("TOKEN")
                .help("Conversions API token (env: PINTEREST_CONVERSION_TOKEN)"),
        )
        .arg(
            Arg::new("ad_account_id")
                .long("ad-account-id")
                .global(true)
                .value_name("ID")
                .help("Default ad account id for ad_accounts/{ad_account_id} paths (env: PINTEREST_AD_ACCOUNT_ID)"),
        )
        .arg(
            Arg::new("base_url")
                .long("base-url")
                .global(true)
                .value_name("URL")
                .help("API base URL (env: PINTEREST_BASE_URL)"),
        )
        .arg(
            Arg::new("pretty")
                .long("pretty")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Pretty-print JSON output"),
        )
        .arg(
            Arg::new("raw_output")
                .long("raw")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Return full API response (do not unwrap items[])"),
        )
        .arg(
            Arg::new("debug")
                .long("debug")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Enable debug logging"),
        )
        .arg(
            Arg::new("timeout")
                .long("timeout")
                .global(true)
                .value_name("SECONDS")
                .value_parser(clap::value_parser!(u64))
                .help("HTTP timeout in seconds"),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .global(true)
                .action(ArgAction::SetTrue)
                .help("Auto-paginate bookmark-based endpoints"),
        )
        .arg(
            Arg::new("max_pages")
                .long("max-pages")
                .global(true)
                .value_name("N")
                .value_parser(clap::value_parser!(u64))
                .help("Max pages to fetch when --all"),
        )
        .arg(
            Arg::new("max_items")
                .long("max-items")
                .global(true)
                .value_name("N")
                .value_parser(clap::value_parser!(u64))
                .help("Max items to fetch when --all"),
        );

    cmd = cmd.subcommand(
        Command::new("list")
            .about("List resources and operations")
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("Emit machine-readable JSON"),
            ),
    );

    cmd = cmd.subcommand(
        Command::new("describe")
            .about("Describe a specific operation")
            .arg(Arg::new("resource").required(true))
            .arg(Arg::new("op").required(true))
            .arg(
                Arg::new("json")
                    .long("json")
                    .action(ArgAction::SetTrue)
                    .help("Emit machine-readable JSON"),
            ),
    );

    cmd = cmd.subcommand(
        Command::new("tree").about("Show full command tree").arg(
            Arg::new("json")
                .long("json")
                .action(ArgAction::SetTrue)
                .help("Emit machine-readable JSON"),
        ),
    );

    cmd = cmd.subcommand(
        Command::new("raw")
            .about("Make a raw API call")
            .arg(Arg::new("method").required(true))
            .arg(Arg::new("path").required(true))
            .arg(
                Arg::new("auth")
                    .long("auth")
                    .value_name("bearer|basic|conversion")
                    .default_value("bearer"),
            )
            .arg(
                Arg::new("params")
                    .long("params")
                    .value_name("JSON")
                    .help("JSON object of query parameters"),
            )
            .arg(
                Arg::new("body")
                    .long("body")
                    .value_name("JSON|@FILE|URL|S3")
                    .help("JSON request body (string or source)"),
            )
            .arg(
                Arg::new("form")
                    .long("form")
                    .value_name("JSON|@FILE|URL|S3")
                    .help("Form body as JSON object (for application/x-www-form-urlencoded)"),
            ),
    );

    for resource in &tree.resources {
        let mut res_cmd = Command::new(resource.name.clone())
            .about(resource.name.clone())
            .subcommand_required(true)
            .arg_required_else_help(true);

        for op in &resource.ops {
            let mut op_cmd =
                Command::new(op.name.clone()).about(op.summary.clone().unwrap_or_default());
            op_cmd = op_cmd.arg(
                Arg::new("params")
                    .long("params")
                    .value_name("JSON")
                    .help("JSON object of query parameters"),
            );
            op_cmd = op_cmd.arg(
                Arg::new("body")
                    .long("body")
                    .value_name("JSON|@FILE|URL|S3")
                    .help("JSON request body (string or source)"),
            );
            op_cmd = op_cmd.arg(
                Arg::new("form")
                    .long("form")
                    .value_name("JSON|@FILE|URL|S3")
                    .help("Form body as JSON object (for application/x-www-form-urlencoded)"),
            );
            for param in &op.params {
                op_cmd = op_cmd.arg(build_param_arg(param));
            }
            res_cmd = res_cmd.subcommand(op_cmd);
        }

        if resource.name == "media" {
            res_cmd = res_cmd.subcommand(
                Command::new("upload")
                    .about("Register + upload media to Pinterest (S3) and optionally wait for processing")
                    .arg(
                        Arg::new("media_type")
                            .long("media-type")
                            .value_name("image|video")
                            .required(true),
                    )
                    .arg(
                        Arg::new("file")
                            .long("file")
                            .value_name("FILE|URL|S3")
                            .required(true),
                    )
                    .arg(
                        Arg::new("wait")
                            .long("wait")
                            .action(ArgAction::SetTrue)
                            .help("Wait for processing to complete"),
                    ),
            );
        }

        cmd = cmd.subcommand(res_cmd);
    }

    cmd
}

fn build_param_arg(param: &ParamDef) -> Arg {
    let mut arg = Arg::new(param_key(param))
        .long(param.flag.clone())
        .value_name(param_value_name(param));

    if param.schema_type == "array" {
        arg = arg.action(ArgAction::Append);
    }

    if param.location == "path" && param.required && param.name != "ad_account_id" {
        arg = arg.required(true);
    }

    arg
}

fn param_value_name(param: &ParamDef) -> String {
    if param.style.as_deref() == Some("deepObject") {
        return "JSON".to_string();
    }
    if param.schema_type == "array" {
        return param
            .items_type
            .clone()
            .unwrap_or_else(|| "value".to_string());
    }
    param.schema_type.clone()
}

fn param_key(param: &ParamDef) -> String {
    format!("param__{}", param.name)
}

fn handle_list(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    if matches.get_flag("json") {
        let mut out = Vec::new();
        for res in &tree.resources {
            let ops: Vec<String> = res.ops.iter().map(|op| op.name.clone()).collect();
            out.push(serde_json::json!({"resource": res.name, "ops": ops}));
        }
        write_json(&Value::Array(out), true)?;
        return Ok(());
    }

    for res in &tree.resources {
        write_stdout_line(&res.name)?;
        for op in &res.ops {
            write_stdout_line(&format!("  {}", op.name))?;
        }
    }
    Ok(())
}

fn handle_describe(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    let resource = matches
        .get_one::<String>("resource")
        .ok_or_else(|| anyhow!("resource required"))?;
    let op_name = matches
        .get_one::<String>("op")
        .ok_or_else(|| anyhow!("operation required"))?;

    let op = find_op(tree, resource, op_name)
        .ok_or_else(|| anyhow!("unknown command {resource} {op_name}"))?;

    if matches.get_flag("json") {
        write_json(&serde_json::to_value(op)?, true)?;
        return Ok(());
    }

    write_stdout_line(&format!("{} {}", resource, op.name))?;
    write_stdout_line(&format!("  method: {}", op.method))?;
    write_stdout_line(&format!("  path: {}", op.path))?;
    write_stdout_line(&format!("  paginated: {}", op.paginated))?;

    if !op.security.is_empty() {
        let schemes: Vec<String> = op
            .security
            .iter()
            .flat_map(|req| req.keys().cloned().collect::<Vec<_>>())
            .collect();
        write_stdout_line(&format!("  auth: {}", schemes.join(" | ")))?;
    }

    if let Some(rb) = &op.request_body {
        write_stdout_line(&format!("  request_body: required={}", rb.required))?;
        if !rb.content_types.is_empty() {
            write_stdout_line(&format!(
                "    content_types: {}",
                rb.content_types.join(", ")
            ))?;
        }
    }

    if !op.params.is_empty() {
        write_stdout_line("  params:")?;
        for param in &op.params {
            write_stdout_line(&format!(
                "    --{}  {} ({}, required={})",
                param.flag,
                param_value_name(param),
                param.location,
                param.required
            ))?;
        }
    }

    Ok(())
}

fn handle_tree(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    if matches.get_flag("json") {
        write_json(&serde_json::to_value(tree)?, true)?;
        return Ok(());
    }
    write_stdout_line("Run with --json for machine-readable output.")?;
    Ok(())
}

fn handle_raw(tree: &CommandTree, matches: &clap::ArgMatches) -> Result<()> {
    let config = load_config(tree, matches)?;
    setup_logging(matches.get_flag("debug"))?;
    let client = PinterestClient::new(config.base_url.clone(), config.timeout)?;

    let method = matches
        .get_one::<String>("method")
        .ok_or_else(|| anyhow!("method required"))?
        .to_ascii_uppercase();
    let path = matches
        .get_one::<String>("path")
        .ok_or_else(|| anyhow!("path required"))?;

    let auth = match matches
        .get_one::<String>("auth")
        .map(|v| v.as_str())
        .unwrap_or("bearer")
    {
        "basic" => Auth::Basic {
            username: config
                .client_id
                .clone()
                .ok_or_else(|| anyhow!("PINTEREST_CLIENT_ID missing"))?,
            password: config
                .client_secret
                .clone()
                .ok_or_else(|| anyhow!("PINTEREST_CLIENT_SECRET missing"))?,
        },
        "conversion" => Auth::Bearer(
            config
                .conversion_token
                .clone()
                .ok_or_else(|| anyhow!("PINTEREST_CONVERSION_TOKEN missing"))?,
        ),
        _ => Auth::Bearer(
            config
                .access_token
                .clone()
                .ok_or_else(|| anyhow!("PINTEREST_ACCESS_TOKEN missing"))?,
        ),
    };

    let params_json = matches.get_one::<String>("params");
    let query = parse_params_json(params_json, &[])?;

    let body = if let Some(raw) = matches.get_one::<String>("body") {
        Some(Body::Json(parse_json_source(raw)?))
    } else if let Some(raw) = matches.get_one::<String>("form") {
        Some(Body::Form(parse_form_source(raw)?))
    } else {
        None
    };

    let url = client.build_url(path);
    let resp = client.request(&method, &url, &auth, &query, body)?;
    write_json(&resp, matches.get_flag("pretty"))?;
    Ok(())
}

fn handle_media_upload(
    client: &PinterestClient,
    config: &Config,
    matches: &clap::ArgMatches,
    pretty: bool,
) -> Result<()> {
    let token = config
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("PINTEREST_ACCESS_TOKEN missing"))?;
    let auth = Auth::Bearer(token);

    let media_type = matches
        .get_one::<String>("media_type")
        .ok_or_else(|| anyhow!("--media-type required"))?;
    let file = matches
        .get_one::<String>("file")
        .ok_or_else(|| anyhow!("--file required"))?;
    let wait = matches.get_flag("wait");

    let file = sources::resolve_source(file)?;
    let resp = media_upload::upload_media(client, &auth, media_type, &file, wait)?;
    write_json(&resp, pretty)?;
    Ok(())
}

fn find_op<'a>(tree: &'a CommandTree, res: &str, op: &str) -> Option<&'a Operation> {
    tree.resources
        .iter()
        .find(|r| r.name == res)
        .and_then(|r| r.ops.iter().find(|o| o.name == op))
}

fn select_auth(op: &Operation, config: &Config) -> Result<Auth> {
    if op.security.iter().any(|req| req.contains_key("basic")) {
        return Ok(Auth::Basic {
            username: config
                .client_id
                .clone()
                .ok_or_else(|| anyhow!("PINTEREST_CLIENT_ID missing"))?,
            password: config
                .client_secret
                .clone()
                .ok_or_else(|| anyhow!("PINTEREST_CLIENT_SECRET missing"))?,
        });
    }

    if op
        .security
        .iter()
        .any(|req| req.contains_key("conversion_token"))
    {
        if let Some(token) = &config.conversion_token {
            return Ok(Auth::Bearer(token.clone()));
        }
    }

    let token = config
        .access_token
        .clone()
        .ok_or_else(|| anyhow!("PINTEREST_ACCESS_TOKEN missing"))?;
    Ok(Auth::Bearer(token))
}

fn build_path(op: &Operation, matches: &clap::ArgMatches, config: &Config) -> Result<String> {
    let mut path = op.path.clone();

    for param in op.params.iter().filter(|p| p.location == "path") {
        let value = matches
            .get_one::<String>(&param_key(param))
            .cloned()
            .or_else(|| {
                if param.name == "ad_account_id" {
                    config.ad_account_id.clone()
                } else {
                    None
                }
            });

        let Some(value) = value else {
            return Err(anyhow!("missing required path param: {}", param.name));
        };

        let encoded = urlencoding::encode(&value);
        path = path.replace(&format!("{{{}}}", param.name), encoded.as_ref());
    }

    if path.contains('{') {
        return Err(anyhow!("unresolved path template: {}", op.path));
    }

    Ok(path)
}

fn build_query_params(op: &Operation, matches: &clap::ArgMatches) -> Result<Vec<(String, String)>> {
    let params_json = matches.get_one::<String>("params");
    let mut out = parse_params_json(params_json, &op.params)?;

    for param in op.params.iter().filter(|p| p.location == "query") {
        let key = param.name.clone();

        if param.schema_type == "array" {
            if let Some(values) = matches.get_many::<String>(&param_key(param)) {
                remove_query_key(&mut out, &key, param.style.as_deref());
                for v in values {
                    out.push((key.clone(), v.clone()));
                }
            }
            continue;
        }

        if param.style.as_deref() == Some("deepObject") {
            if let Some(raw) = matches.get_one::<String>(&param_key(param)) {
                remove_query_key(&mut out, &key, param.style.as_deref());
                let value = parse_json_source(raw)?;
                out.extend(encode_deep_object(&key, &value)?);
            }
            continue;
        }

        if let Some(value) = matches.get_one::<String>(&param_key(param)) {
            remove_query_key(&mut out, &key, param.style.as_deref());
            out.push((key, value.clone()));
        }
    }

    Ok(out)
}

fn remove_query_key(out: &mut Vec<(String, String)>, key: &str, style: Option<&str>) {
    if style == Some("deepObject") {
        let prefix = format!("{key}[");
        out.retain(|(k, _)| !(k == key || k.starts_with(&prefix)));
        return;
    }
    out.retain(|(k, _)| k != key);
}

fn parse_params_json(
    params_json: Option<&String>,
    params: &[ParamDef],
) -> Result<Vec<(String, String)>> {
    let Some(raw) = params_json else {
        return Ok(Vec::new());
    };
    let value: Value = serde_json::from_str(raw).context("invalid JSON for --params")?;
    let Value::Object(map) = value else {
        return Err(anyhow!("--params must be a JSON object"));
    };

    let mut out = Vec::new();
    for (k, v) in map {
        let style = params
            .iter()
            .find(|p| p.location == "query" && p.name == k)
            .and_then(|p| p.style.as_deref());

        if style == Some("deepObject") {
            out.extend(encode_deep_object(&k, &v)?);
            continue;
        }

        match v {
            Value::Array(values) => {
                for item in values {
                    out.push((k.clone(), json_value_to_string(&item)?));
                }
            }
            _ => out.push((k, json_value_to_string(&v)?)),
        }
    }
    Ok(out)
}

fn encode_deep_object(prefix: &str, value: &Value) -> Result<Vec<(String, String)>> {
    let Value::Object(map) = value else {
        return Err(anyhow!("deepObject param must be a JSON object"));
    };

    fn walk(out: &mut Vec<(String, String)>, key: &str, value: &Value) -> Result<()> {
        match value {
            Value::Null => Ok(()),
            Value::Bool(_) | Value::Number(_) | Value::String(_) => {
                out.push((key.to_string(), json_value_to_string(value)?));
                Ok(())
            }
            Value::Array(items) => {
                for item in items {
                    out.push((key.to_string(), json_value_to_string(item)?));
                }
                Ok(())
            }
            Value::Object(map) => {
                for (k, v) in map {
                    walk(out, &format!("{key}[{k}]"), v)?;
                }
                Ok(())
            }
        }
    }

    let mut out = Vec::new();
    for (k, v) in map {
        walk(&mut out, &format!("{prefix}[{k}]"), v)?;
    }
    Ok(out)
}

fn build_body(op: &Operation, matches: &clap::ArgMatches) -> Result<Option<Body>> {
    let body_arg = matches.get_one::<String>("body");
    let form_arg = matches.get_one::<String>("form");

    let Some(rb) = &op.request_body else {
        if body_arg.is_some() || form_arg.is_some() {
            return Err(anyhow!("request body not supported for this operation"));
        }
        return Ok(None);
    };

    if rb.content_types.iter().any(|ct| ct == "application/json") {
        let Some(raw) = body_arg else {
            if rb.required {
                return Err(anyhow!("--body required"));
            }
            return Ok(None);
        };
        return Ok(Some(Body::Json(parse_json_source(raw)?)));
    }

    if rb
        .content_types
        .iter()
        .any(|ct| ct == "application/x-www-form-urlencoded")
    {
        let Some(raw) = form_arg else {
            if rb.required {
                return Err(anyhow!("--form required"));
            }
            return Ok(None);
        };
        return Ok(Some(Body::Form(parse_form_source(raw)?)));
    }

    Err(anyhow!(
        "unsupported request content types: {}",
        rb.content_types.join(", ")
    ))
}

fn parse_json_source(raw: &str) -> Result<Value> {
    let text = if sources::looks_like_source(raw) {
        sources::read_source_to_string(raw)?
    } else {
        raw.to_string()
    };
    serde_json::from_str(&text).context("invalid JSON")
}

fn parse_form_source(raw: &str) -> Result<Vec<(String, String)>> {
    let text = if sources::looks_like_source(raw) {
        sources::read_source_to_string(raw)?
    } else {
        raw.to_string()
    };
    let value: Value = serde_json::from_str(&text).context("invalid JSON for --form")?;
    let Value::Object(map) = value else {
        return Err(anyhow!("--form must be a JSON object"));
    };

    let mut out = Vec::new();
    for (k, v) in map {
        match v {
            Value::Array(values) => {
                for item in values {
                    out.push((k.clone(), json_value_to_string(&item)?));
                }
            }
            _ => out.push((k, json_value_to_string(&v)?)),
        }
    }
    Ok(out)
}

fn json_value_to_string(value: &Value) -> Result<String> {
    match value {
        Value::String(v) => Ok(v.clone()),
        _ => Ok(serde_json::to_string(value)?),
    }
}

fn write_json(value: &Value, pretty: bool) -> Result<()> {
    if pretty {
        write_stdout_line(&serde_json::to_string_pretty(value)?)
    } else {
        write_stdout_line(&serde_json::to_string(value)?)
    }
}

fn write_stdout_line(value: &str) -> Result<()> {
    let mut out = std::io::stdout().lock();
    if let Err(err) = out.write_all(value.as_bytes()) {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(err.into());
    }
    if let Err(err) = out.write_all(b"\n") {
        if err.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        return Err(err.into());
    }
    Ok(())
}
