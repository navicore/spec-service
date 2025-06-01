#!/bin/bash

# Generate architecture diagrams from PlantUML source
# Requires: plantuml (brew install plantuml)

echo "Generating architecture diagrams..."

# Check if plantuml is installed
if ! command -v plantuml &> /dev/null; then
    echo "PlantUML is not installed. Install with: brew install plantuml"
    exit 1
fi

# Generate PNG diagrams
plantuml -tpng architecture-diagrams.puml

# Generate SVG diagrams (better for documentation)
plantuml -tsvg architecture-diagrams.puml

echo "Diagrams generated:"
ls -la *.png *.svg 2>/dev/null || echo "No diagrams found. Check for errors above."

echo ""
echo "To view in terminal (requires iTerm2 or similar):"
echo "  imgcat building-block-diagram.png"
echo ""
echo "To include in markdown:"
echo "  ![Building Block Diagram](docs/building-block-diagram.png)"