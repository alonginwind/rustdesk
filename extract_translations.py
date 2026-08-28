#!/usr/bin/env python3
"""Extract translations from src/lang/*.rs and generate pure JS bridge."""
import re
import json
import os
import base64
from datetime import datetime, timezone

try:
    import rjsmin
    HAS_RJS = True
except ImportError:
    HAS_RJS = False

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
LANG_DIR = os.path.join(SCRIPT_DIR, 'src', 'lang')
TEMPLATE = os.path.join(SCRIPT_DIR, 'flutter', 'web', 'load_bridge.tpl.js')
OUTPUT = os.path.join(SCRIPT_DIR, 'flutter', 'web', 'load_bridge.js')
CARGO_TOML = os.path.join(SCRIPT_DIR, 'Cargo.toml')

# Regex: ("key", "value") with escaped char support
TUPLE_RE = re.compile(r'\(\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\)')

# File prefix -> locale code (Chinese + English fallback only)
FILE_TO_LOCALE = {
    'cn': 'zh-cn',
    'en': 'en',
}

# Read version from Cargo.toml
version = 'unknown'
if os.path.exists(CARGO_TOML):
    with open(CARGO_TOML, encoding='utf-8') as f:
        for line in f:
            m = re.match(r'^version\s*=\s*"([^"]+)"', line.strip())
            if m:
                version = m.group(1)
                break

build_date = datetime.now(timezone.utc).strftime('%Y-%m-%d %H:%M:%S')

# Extract translations
result = {}
for prefix, locale in sorted(FILE_TO_LOCALE.items()):
    filepath = os.path.join(LANG_DIR, f'{prefix}.rs')
    if not os.path.exists(filepath):
        print(f'  SKIP {prefix}.rs (not found)')
        continue
    translations = {}
    with open(filepath, encoding='utf-8') as f:
        for line in f:
            m = TUPLE_RE.search(line)
            if m:
                key, value = m.group(1), m.group(2)
                if value:
                    translations[key] = value
    result[locale] = translations
    print(f'  {prefix}.rs -> {locale}: {len(translations)} translations')

# Encode translations as base64
raw = json.dumps(result, ensure_ascii=False, separators=(',', ':'))
encoded = base64.b64encode(raw.encode('utf-8')).decode('ascii')

# Read template and replace placeholders
with open(TEMPLATE, encoding='utf-8') as f:
    tpl = f.read()

output = tpl.replace('__TRANSLATIONS_DATA__', encoded)
output = output.replace('__VERSION__', version)
output = output.replace('__BUILD_DATE__', build_date)

# Obfuscate: minify JS (remove comments, collapse whitespace)
if HAS_RJS:
    output = rjsmin.jsmin(output, keep_bang_comments=False)

with open(OUTPUT, 'w', encoding='utf-8') as f:
    f.write(output)

total = sum(len(v) for v in result.values())
size_kb = os.path.getsize(OUTPUT) / 1024
print(f'\nGenerated {OUTPUT}')
print(f'Version: {version}, Build date: {build_date}')
print(f'{len(result)} languages, {total} translations, {size_kb:.0f} KB')
if HAS_RJS:
    print('Obfuscation: enabled (rjsmin)')
else:
    print('Obfuscation: disabled (pip install rjsmin to enable)')
