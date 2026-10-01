#!/bin/sh
set -eu

if [ -z "${IRA_MASTER_KEY:-}" ] && [ -n "${LEO_MASTER_KEY:-}" ]; then
    IRA_MASTER_KEY=$LEO_MASTER_KEY
    export IRA_MASTER_KEY
fi

if [ -z "${IRA_MASTER_KEY:-}" ]; then
    key_file="${HOME:-/home/ira}/.ira/master.key"
    mkdir -p "$(dirname "$key_file")"
    if [ ! -s "$key_file" ]; then
        umask 077
        temp_file="$key_file.tmp.$$"
        head -c 32 /dev/urandom | base64 | tr -d '\r\n' > "$temp_file"
        printf '\n' >> "$temp_file"
        mv "$temp_file" "$key_file"
    fi
    IRA_MASTER_KEY=$(tr -d '\r\n' < "$key_file")
    export IRA_MASTER_KEY
fi

unset LEO_MASTER_KEY
exec "$@"
