# Agent Hand Logo Assets

## Files

### Full Logo (with text)
- `agent-hand-full.jpg` / `.png` (1408x768) - Original full logo with "AGENT>-HAND" text

### Icon Only (no text)
- `icon-only.jpg` / `.png` (600x600) - Cropped to show only the hand + terminal windows

### Square Icons (various sizes)
- `agent-hand-icon-square.jpg` (768x768) - Square version
- `icon-512.jpg` / `.png` (512x512) - Large icon
- `icon-256.jpg` / `.png` (256x256) - Medium icon
- `icon-128.jpg` / `.png` (128x128) - Standard icon
- `icon-64.jpg` / `.png` (64x64) - Small icon
- `icon-32.jpg` / `.png` (32x32) - Tiny icon
- `icon-16.jpg` / `.png` (16x16) - Favicon size

### Windows Icon
- `agent-hand.ico` - Multi-resolution Windows icon (16, 32, 64, 128, 256px)

## Usage Recommendations

### GitHub Repository
- **Social preview**: Use `agent-hand-full.jpg` (1280x640 recommended, current is 1408x768 - close enough)
- **README header**: Use `agent-hand-full.jpg`
- **Avatar/Icon**: Use `icon-256.jpg` or `icon-512.jpg`

### Documentation
- **Hero image**: `agent-hand-full.jpg`
- **Inline icon**: `icon-64.jpg` or `icon-128.jpg`

### Application
- **macOS .app icon**: Use `icon-512.jpg` (convert to .icns)
- **Windows .exe icon**: Use `icon-256.jpg` (convert to .ico)
- **Linux desktop**: Use `icon-256.jpg` or `icon-512.jpg`

### Web/Social Media
- **Favicon**: `icon-32.jpg` or `icon-16.jpg` (convert to .ico or .png)
- **Twitter/X**: `icon-512.jpg` (400x400 recommended)
- **LinkedIn**: `icon-512.jpg` (300x300 recommended)

## Design Elements

- **Primary color**: Rust orange (#F74C00)
- **Secondary colors**: Terminal green, cyan blue, dark charcoal
- **Style**: Geometric, minimalist, developer-focused
- **Concept**: Hand controlling multiple terminal sessions (tmux panes)

## Converting to Other Formats

### Convert to PNG (lossless)
```bash
sips -s format png icon-512.jpg --out icon-512.png
```

### Convert to ICNS (macOS app icon)
```bash
# Create iconset directory
mkdir agent-hand.iconset
cp icon-16.jpg agent-hand.iconset/icon_16x16.png
cp icon-32.jpg agent-hand.iconset/icon_16x16@2x.png
cp icon-32.jpg agent-hand.iconset/icon_32x32.png
cp icon-64.jpg agent-hand.iconset/icon_32x32@2x.png
cp icon-128.jpg agent-hand.iconset/icon_128x128.png
cp icon-256.jpg agent-hand.iconset/icon_128x128@2x.png
cp icon-256.jpg agent-hand.iconset/icon_256x256.png
cp icon-512.jpg agent-hand.iconset/icon_256x256@2x.png
cp icon-512.jpg agent-hand.iconset/icon_512x512.png

# Convert to icns
iconutil -c icns agent-hand.iconset
```

### Convert to ICO (Windows icon)
```bash
# Requires ImageMagick
convert icon-16.jpg icon-32.jpg icon-64.jpg icon-128.jpg icon-256.jpg agent-hand.ico
```

## License

Same as the main project (MIT).
