use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct CommandTree {
    pub version: u32,
    pub api_version: String,
    pub base_url: String,
    pub resources: Vec<Resource>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct Resource {
    pub name: String,
    pub ops: Vec<Operation>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct Operation {
    pub name: String,
    pub method: String,
    pub path: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub paginated: bool,
    pub security: Vec<BTreeMap<String, Vec<String>>>,
    pub params: Vec<ParamDef>,
    pub request_body: Option<RequestBodyDef>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct ParamDef {
    pub name: String,
    pub flag: String,
    #[serde(rename = "in")]
    pub location: String,
    pub required: bool,
    pub style: Option<String>,
    pub explode: Option<bool>,
    pub schema_type: String,
    pub items_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub struct RequestBodyDef {
    pub required: bool,
    pub content_types: Vec<String>,
}

pub fn load_command_tree() -> CommandTree {
    let raw = include_str!("../schemas/command_tree.json");
    serde_json::from_str(raw).expect("invalid schemas/command_tree.json")
}
