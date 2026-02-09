#!/usr/bin/env python3
import argparse
import json
import re
from collections import defaultdict
from typing import Any, Dict, List, Optional, Tuple


def to_kebab(value: str) -> str:
    # Handles snake_case and simple CamelCase.
    value = value.replace("_", "-")
    value = re.sub(r"(.)([A-Z][a-z]+)", r"\1-\2", value)
    value = re.sub(r"([a-z0-9])([A-Z])", r"\1-\2", value)
    value = re.sub(r"-+", "-", value)
    return value.strip("-").lower()


def resolve_ref(doc: Dict[str, Any], ref: str) -> Any:
    if not ref.startswith("#/"):
        raise RuntimeError(f"unsupported ref: {ref}")
    cur: Any = doc
    for part in ref[2:].split("/"):
        cur = cur[part]
    return cur


def normalize_op_id(op_id: str, tags: List[str]) -> Tuple[str, str]:
    parts = op_id.split("/")
    if len(parts) == 1:
        res = tags[0] if tags else "misc"
        op = parts[0]
    elif len(parts) == 2:
        res, op = parts
    else:
        res = parts[0]
        op = "-".join(parts[1:])
    return to_kebab(res), to_kebab(op)


def schema_type(doc: Dict[str, Any], schema: Dict[str, Any]) -> Tuple[str, Optional[str]]:
    if "$ref" in schema:
        schema = resolve_ref(doc, schema["$ref"])
    typ = schema.get("type") or "string"
    if typ != "array":
        return typ, None
    items = schema.get("items") or {}
    if "$ref" in items:
        items = resolve_ref(doc, items["$ref"])
    return "array", (items.get("type") or "string")


def parse_param(doc: Dict[str, Any], param: Dict[str, Any]) -> Dict[str, Any]:
    schema = param.get("schema") or {}
    typ, items_typ = schema_type(doc, schema)
    return {
        "name": param["name"],
        "flag": param["name"].replace("_", "-"),
        "in": param["in"],
        "required": bool(param.get("required", False)),
        "style": param.get("style"),
        "explode": param.get("explode"),
        "schema_type": typ,
        "items_type": items_typ,
    }


def parse_request_body(doc: Dict[str, Any], request_body: Any) -> Optional[Dict[str, Any]]:
    if not request_body:
        return None
    if isinstance(request_body, dict) and "$ref" in request_body:
        request_body = resolve_ref(doc, request_body["$ref"])
    content = request_body.get("content") or {}
    return {
        "required": bool(request_body.get("required", False)),
        "content_types": sorted(content.keys()),
    }


def pick_response_schema(doc: Dict[str, Any], op: Dict[str, Any]) -> Optional[Dict[str, Any]]:
    responses = op.get("responses") or {}
    for code in ("200", "201", "202"):
        resp = responses.get(code)
        if not resp:
            continue
        content = (resp.get("content") or {}).get("application/json") or {}
        schema = content.get("schema")
        if schema:
            return schema
    return None


def is_paginated(doc: Dict[str, Any], schema: Optional[Dict[str, Any]]) -> bool:
    if not schema:
        return False
    if "$ref" in schema:
        ref = schema["$ref"]
        if ref.endswith("/Paginated"):
            return True
        return is_paginated(doc, resolve_ref(doc, ref))
    if "allOf" in schema:
        return any(is_paginated(doc, s) for s in schema.get("allOf") or [])
    return False


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--openapi", required=True, help="path to openapi.json")
    parser.add_argument("--out", required=True, help="output path (schemas/command_tree.json)")
    args = parser.parse_args()

    with open(args.openapi, "r", encoding="utf-8") as f:
        doc = json.load(f)

    api_version = (doc.get("info") or {}).get("version") or ""
    base_url = ""
    servers = doc.get("servers") or []
    if servers:
        base_url = (servers[0] or {}).get("url") or ""
    if not base_url:
        base_url = "https://api.pinterest.com/v5"

    global_security = doc.get("security") or []
    resources: Dict[str, List[Dict[str, Any]]] = defaultdict(list)

    for path, path_item in (doc.get("paths") or {}).items():
        for method, op in (path_item or {}).items():
            if method.startswith("x-"):
                continue
            if method not in ("get", "post", "put", "patch", "delete"):
                continue

            op_id = op.get("operationId")
            if not op_id:
                continue
            tags = op.get("tags") or []
            res_name, op_name = normalize_op_id(op_id, tags)

            params: List[Dict[str, Any]] = []
            for param in (path_item.get("parameters") or []) + (op.get("parameters") or []):
                if "$ref" in param:
                    param = resolve_ref(doc, param["$ref"])
                params.append(parse_param(doc, param))

            # Stable ordering: path params first, then query params, then by name.
            params.sort(key=lambda p: (p["in"] != "path", p["in"], p["name"]))

            rb = parse_request_body(doc, op.get("requestBody"))
            resp_schema = pick_response_schema(doc, op)
            paginated = is_paginated(doc, resp_schema)

            security = op.get("security")
            if security is None:
                security = global_security

            resources[res_name].append(
                {
                    "name": op_name,
                    "method": method.upper(),
                    "path": path,
                    "summary": op.get("summary"),
                    "tags": tags,
                    "paginated": paginated,
                    "security": security,
                    "params": params,
                    "request_body": rb,
                }
            )

    out_resources = []
    for res_name in sorted(resources.keys()):
        ops = resources[res_name]
        # Ensure op names unique within a resource.
        seen: Dict[str, int] = {}
        for op in ops:
            base = op["name"]
            if base in seen:
                seen[base] += 1
                op["name"] = f"{base}-{seen[base]}"
            else:
                seen[base] = 1

        ops.sort(key=lambda o: o["name"])
        out_resources.append({"name": res_name, "ops": ops})

    out = {
        "version": 1,
        "api_version": api_version,
        "base_url": base_url,
        "resources": out_resources,
    }

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=2, sort_keys=False)
        f.write("\n")


if __name__ == "__main__":
    main()

