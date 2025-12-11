# Stream Deck Icons

This directory should contain PNG icons for the plugin.

## Required Icons

| File | Size | Description |
|------|------|-------------|
| `plugin.png` | 256x256 | Plugin icon in Stream Deck store |
| `plugin@2x.png` | 512x512 | High-DPI plugin icon |
| `action.png` | 20x20 | Action icon in action list |
| `action@2x.png` | 40x40 | High-DPI action icon |
| `category.png` | 28x28 | Category icon |
| `category@2x.png` | 56x56 | High-DPI category icon |
| `state-default.png` | 72x72 | Default button state |
| `state-default@2x.png` | 144x144 | High-DPI default state |
| `state-active.png` | 72x72 | Active/highlighted button state |
| `state-active@2x.png` | 144x144 | High-DPI active state |

## Icon Guidelines

- Use transparent backgrounds for action/state icons
- Plugin icon should have a solid background
- Keep designs simple and recognizable at small sizes
- Use the Lamzu brand colors if available

## Placeholder SVG

The `plugin.svg` file is a placeholder that can be converted to PNG using:
- Inkscape: `inkscape -w 256 -h 256 plugin.svg -o plugin.png`
- ImageMagick: `convert -resize 256x256 plugin.svg plugin.png`
