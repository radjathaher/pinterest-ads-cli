# pinterest-ads-cli

Auto-generated Pinterest Ads API CLI (Pinterest REST API v5). Designed for LLM discovery + scripting.

## Install

### Install script (macOS + Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/radjathaher/pinterest-ads-cli/main/scripts/install.sh | bash
```

### Download from GitHub releases

Grab the latest `pinterest-ads-cli-<version>-<os>-<arch>.tar.gz`, unpack, and move `pinterest-ads` to your PATH.

## Auth / Env

Most endpoints require:

```bash
export PINTEREST_ACCESS_TOKEN="..."
```

Optional defaults:

```bash
export PINTEREST_AD_ACCOUNT_ID="1234567890"   # used when ad_account_id path param omitted
export PINTEREST_BASE_URL="https://api.pinterest.com/v5"
```

Token helpers (needed to generate tokens via `/oauth/token`):

```bash
export PINTEREST_CLIENT_ID="..."
export PINTEREST_CLIENT_SECRET="..."
```

Conversions API token (only for `/ad_accounts/{ad_account_id}/events` if using conversion token auth):

```bash
export PINTEREST_CONVERSION_TOKEN="..."
```

### How to get these

1. Create a Pinterest Developer app.
2. Get `PINTEREST_CLIENT_ID` + `PINTEREST_CLIENT_SECRET` from the app settings.
3. Generate an access token:

Client Credentials grant:

```bash
pinterest-ads oauth token --form '{"grant_type":"client_credentials","scope":"ads:read"}' --pretty
```

Authorization Code grant: follow Pinterest docs to get `code` + `redirect_uri`, then:

```bash
pinterest-ads oauth token --form '{"grant_type":"authorization_code","code":"...","redirect_uri":"...","continuous_refresh":true}' --pretty
```

Note: `continuous_refresh=true` only applies to apps created before 2025-09-25.

4. Get your ad account id:

```bash
pinterest-ads ad-accounts list --pretty
```

## Discovery

```bash
pinterest-ads list --json
pinterest-ads describe campaigns list --json
pinterest-ads tree --json
```

## Examples

List campaigns:

```bash
pinterest-ads campaigns list --ad-account-id 123 --page-size 10 --pretty
```

Create campaigns (request body is a JSON array; max 30):

```bash
pinterest-ads campaigns create --ad-account-id 123 --body @./campaigns.json --pretty
```

Bookmark pagination:

```bash
pinterest-ads campaigns list --ad-account-id 123 --all --max-items 500 --pretty
```

Media upload (register + upload + optional wait):

```bash
pinterest-ads media upload --media-type video --file ./video.mp4 --wait --pretty
```

Raw call:

```bash
pinterest-ads raw GET /ad_accounts --params '{"page_size":10}' --pretty
```

## Regenerate command tree

```bash
tools/fetch_openapi.py --out schemas
tools/gen_command_tree.py --openapi schemas/openapi.json --out schemas/command_tree.json
cargo build
```

Note: GitHub secret scanning may flag some example strings in `schemas/openapi.json` as credentials (false positive).
