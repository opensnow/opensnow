#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

CHART_DIR="$PROJECT_DIR/deploy/helm"
OUTPUT_DIR="$PROJECT_DIR/site/public/charts"

# Get version from Chart.yaml if not provided
VERSION=${1:-$(grep '^version:' "$CHART_DIR/Chart.yaml" | cut -d' ' -f2)}
SITE_URL=${SITE_URL:-"https://charts.opensnow.io"}

echo "Packaging helm chart version $VERSION"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Package the chart
helm package "$CHART_DIR" --version "$VERSION" --destination "$OUTPUT_DIR/"

# Create/update index
helm repo index "$OUTPUT_DIR" --url "$SITE_URL/charts"

echo "Chart packaged successfully"
echo "  Archive: $OUTPUT_DIR/opensnow-$VERSION.tgz"
echo "  Index:   $OUTPUT_DIR/index.yaml"
