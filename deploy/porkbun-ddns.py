"""Keep only the existing ira.markus.dev A record updated on Porkbun."""

import ipaddress
import json
import time
import urllib.error
import urllib.request

DOMAIN = "markus.dev"
HOST = "ira.markus.dev"
BASE = f"https://api.porkbun.com/api/json/v3/dns"
KEY_FILE = "/run/secrets/porkbun-dns.json"


def request(url, keys, data=None):
    headers = {
        "X-API-Key": keys["apikey"],
        "X-Secret-API-Key": keys["secretapikey"],
        "Content-Type": "application/json",
    }
    body = json.dumps(data).encode() if data is not None else None
    with urllib.request.urlopen(
        urllib.request.Request(url, data=body, headers=headers, method="POST" if body else "GET"),
        timeout=20,
    ) as response:
        result = json.load(response)
    if result.get("status") != "SUCCESS":
        raise ValueError(f"Porkbun API error: {result.get('message', 'unknown')}")
    if result.get("warnings"):
        raise ValueError(f"Porkbun DNS warnings: {result['warnings']}")
    return result


def update():
    with open(KEY_FILE, encoding="utf-8") as file:
        keys = json.load(file)
    with urllib.request.urlopen("https://api.ipify.org", timeout=20) as response:
        ip = str(ipaddress.IPv4Address(response.read().decode().strip()))
    if not ipaddress.ip_address(ip).is_global:
        raise ValueError("Public IPv4 unavailable; check CG-NAT")
    result = request(f"{BASE}/retrieveByNameType/{DOMAIN}/A/ira", keys)
    records = result.get("records", [])
    if len(records) != 1 or records[0].get("name") != HOST or records[0].get("type") != "A":
        raise ValueError("Expected exactly one existing ira.markus.dev A record; refusing to edit")
    record = records[0]
    if record["content"] == ip:
        return
    request(f"{BASE}/edit/{DOMAIN}/{record['id']}", keys, {"content": ip, "ttl": 600})
    print(f"Updated {HOST} -> {ip}", flush=True)


if __name__ == "__main__":
    while True:
        try:
            update()
        except (OSError, ValueError, KeyError, urllib.error.URLError) as error:
            print(f"DNS update failed: {error}", flush=True)
        time.sleep(300)
