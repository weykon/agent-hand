#!/bin/bash
# Inject JSON-LD schema into index.html <head> AFTER Next.js build
# This avoids the RSC payload duplication that causes Google Rich Results errors
set -e

OUT_DIR="out"
SCHEMA_FILE="schema.json"

if [ ! -f "$SCHEMA_FILE" ]; then
  echo "Error: $SCHEMA_FILE not found"
  exit 1
fi

SCHEMA_CONTENT=$(cat "$SCHEMA_FILE")
SCRIPT_TAG="<script type=\"application/ld+json\">${SCHEMA_CONTENT}</script>"

# Inject into index.html only (homepage schemas)
if [ -f "$OUT_DIR/index.html" ]; then
  # Insert right after <head> or after first <meta> in <head>
  sed -i.bak "s|</head>|${SCRIPT_TAG}</head>|" "$OUT_DIR/index.html"
  rm -f "$OUT_DIR/index.html.bak"
  echo "Injected schema into $OUT_DIR/index.html"
else
  echo "Warning: $OUT_DIR/index.html not found"
fi
