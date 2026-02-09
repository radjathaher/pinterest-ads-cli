#!/usr/bin/env python3
import argparse
import os
from urllib.request import urlopen

OPENAPI_JSON = "https://raw.githubusercontent.com/pinterest/api-description/main/v5/openapi.json"


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", required=True, help="output directory (schemas)")
    args = parser.parse_args()

    os.makedirs(args.out, exist_ok=True)
    out_path = os.path.join(args.out, "openapi.json")
    with urlopen(OPENAPI_JSON) as resp:
        data = resp.read()
    with open(out_path, "wb") as f:
        f.write(data)
    print(out_path)


if __name__ == "__main__":
    main()

