#!/usr/bin/env python3
"""Inject JSON-LD schema into index.html <head> AFTER Next.js build.
This avoids the RSC payload duplication that causes Google Rich Results errors."""
import pathlib

out = pathlib.Path("out/index.html")
schema = pathlib.Path("schema.json").read_text().strip()

html = out.read_text()
tag = f'<script type="application/ld+json">{schema}</script>'
html = html.replace("</head>", f"{tag}</head>", 1)
out.write_text(html)
print(f"Injected schema into {out}")
