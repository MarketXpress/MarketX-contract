#!/usr/bin/env python3
"""Check that error codes in errors.rs match those in sdk/error-codes.ts."""

import re
import sys


def extract_rust_codes(path):
    codes = set()
    with open(path) as f:
        for line in f:
            stripped = line.strip()
            if re.match(r'^\s*[A-Za-z_][A-Za-z0-9_]*\s*=\s*\d+\s*,?\s*$', stripped):
                code = int(re.search(r'=\s*(\d+)', stripped).group(1))
                codes.add(code)
    return codes


def extract_ts_codes(path):
    codes = set()
    with open(path) as f:
        for line in f:
            stripped = line.strip()
            if m := re.match(r'^\s*(\d+)\s*:\s*\{', stripped):
                codes.add(int(m.group(1)))
    return codes


rust_codes = extract_rust_codes('contracts/marketx/src/errors.rs')
ts_codes = extract_ts_codes('sdk/error-codes.ts')

missing_in_ts = sorted(rust_codes - ts_codes)
missing_in_rust = sorted(ts_codes - rust_codes)

if missing_in_ts or missing_in_rust:
    if missing_in_ts:
        print(f'Error: codes in Rust but missing in SDK: {missing_in_ts}')
    if missing_in_rust:
        print(f'Error: codes in SDK but missing in Rust: {missing_in_rust}')
    sys.exit(1)

print('SDK error code parity check passed.')
