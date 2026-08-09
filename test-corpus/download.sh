#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# With no arguments, fetch every corpus. With arguments, fetch only those named:
#   bash download.sh sass-spec
WANTED=("$@")

want() {
    [ ${#WANTED[@]} -eq 0 ] && return 0
    local name
    for name in "${WANTED[@]}"; do
        [ "$name" = "$1" ] && return 0
    done
    return 1
}

# ── Bootstrap ────────────────────────────────────────────────────────
BOOTSTRAP_VERSION="v5.3.3"
BOOTSTRAP_DIR="$SCRIPT_DIR/bootstrap"

if want bootstrap; then
    if [ -d "$BOOTSTRAP_DIR/scss" ]; then
        echo "bootstrap already downloaded at $BOOTSTRAP_DIR/scss"
    else
        echo "downloading bootstrap $BOOTSTRAP_VERSION..."
        TMPDIR=$(mktemp -d)
        curl -sL "https://github.com/twbs/bootstrap/archive/refs/tags/${BOOTSTRAP_VERSION}.tar.gz" \
            | tar xz -C "$TMPDIR"
        mkdir -p "$BOOTSTRAP_DIR"
        mv "$TMPDIR/bootstrap-${BOOTSTRAP_VERSION#v}/scss" "$BOOTSTRAP_DIR/scss"
        rm -rf "$TMPDIR"
        echo "bootstrap scss extracted to $BOOTSTRAP_DIR/scss"
    fi
fi

# ── Foundation ───────────────────────────────────────────────────────
FOUNDATION_VERSION="v6.9.0"
FOUNDATION_DIR="$SCRIPT_DIR/foundation"

if want foundation; then
    if [ -d "$FOUNDATION_DIR/scss" ]; then
        echo "foundation already downloaded at $FOUNDATION_DIR/scss"
    else
        echo "downloading foundation $FOUNDATION_VERSION..."
        TMPDIR=$(mktemp -d)
        curl -sL "https://github.com/foundation/foundation-sites/archive/refs/tags/${FOUNDATION_VERSION}.tar.gz" \
            | tar xz -C "$TMPDIR"
        mkdir -p "$FOUNDATION_DIR"
        mv "$TMPDIR/foundation-sites-${FOUNDATION_VERSION#v}/scss" "$FOUNDATION_DIR/scss"
        rm -rf "$TMPDIR"
        echo "foundation scss extracted to $FOUNDATION_DIR/scss"
    fi
fi

# ── Primer CSS ───────────────────────────────────────────────────────
PRIMER_VERSION="v22.1.0"
PRIMER_DIR="$SCRIPT_DIR/primer"

if want primer; then
    if [ -d "$PRIMER_DIR/src" ]; then
        echo "primer already downloaded at $PRIMER_DIR/src"
    else
        echo "downloading primer $PRIMER_VERSION..."
        TMPDIR=$(mktemp -d)
        curl -sL "https://github.com/primer/css/archive/refs/tags/${PRIMER_VERSION}.tar.gz" \
            | tar xz -C "$TMPDIR"
        mkdir -p "$PRIMER_DIR"
        mv "$TMPDIR/css-${PRIMER_VERSION#v}/src" "$PRIMER_DIR/src"
        rm -rf "$TMPDIR"
        echo "primer scss extracted to $PRIMER_DIR/src"
    fi
fi

# ── Bulma ────────────────────────────────────────────────────────────
BULMA_VERSION="1.0.4"
BULMA_DIR="$SCRIPT_DIR/bulma"

if want bulma; then
    if [ -d "$BULMA_DIR/sass" ]; then
        echo "bulma already downloaded at $BULMA_DIR/sass"
    else
        echo "downloading bulma $BULMA_VERSION..."
        TMPDIR=$(mktemp -d)
        curl -sL "https://github.com/jgthms/bulma/archive/refs/tags/${BULMA_VERSION}.tar.gz" \
            | tar xz -C "$TMPDIR"
        mkdir -p "$BULMA_DIR"
        mv "$TMPDIR/bulma-${BULMA_VERSION}/sass" "$BULMA_DIR/sass"
        rm -rf "$TMPDIR"
        echo "bulma scss extracted to $BULMA_DIR/sass"
    fi
fi

# ── Angular Material ────────────────────────────────────────────────
ANGULAR_VERSION="v21.2.1"
ANGULAR_DIR="$SCRIPT_DIR/angular-material"

if want angular-material; then
    if [ -d "$ANGULAR_DIR/scss" ]; then
        echo "angular-material already downloaded at $ANGULAR_DIR/scss"
    else
        echo "downloading angular material $ANGULAR_VERSION..."
        TMPDIR=$(mktemp -d)
        curl -sL "https://github.com/angular/components/archive/refs/tags/${ANGULAR_VERSION}.tar.gz" \
            | tar xz -C "$TMPDIR"
        mkdir -p "$ANGULAR_DIR/scss"
        # Extract only .scss files from src/material/
        find "$TMPDIR/components-${ANGULAR_VERSION#v}/src/material" -name '*.scss' -print0 \
            | while IFS= read -r -d '' f; do
                rel="${f#$TMPDIR/components-${ANGULAR_VERSION#v}/src/material/}"
                dest="$ANGULAR_DIR/scss/$rel"
                mkdir -p "$(dirname "$dest")"
                mv "$f" "$dest"
            done
        rm -rf "$TMPDIR"
        echo "angular material scss extracted to $ANGULAR_DIR/scss"
    fi
fi

# ── sass-spec ──────────────────────────────────────────────────────
SASS_SPEC_COMMIT="63a0becd13d3639a60669d8b710e75b282415047"
SASS_SPEC_DIR="$SCRIPT_DIR/sass-spec"

if want sass-spec; then
    if [ -d "$SASS_SPEC_DIR/spec" ]; then
        echo "sass-spec already downloaded at $SASS_SPEC_DIR/spec"
    else
        echo "downloading sass-spec (commit ${SASS_SPEC_COMMIT:0:8})..."
        TMPDIR=$(mktemp -d)
        curl -sL "https://github.com/sass/sass-spec/archive/${SASS_SPEC_COMMIT}.tar.gz" \
            | tar xz -C "$TMPDIR"
        mkdir -p "$SASS_SPEC_DIR"
        mv "$TMPDIR/sass-spec-${SASS_SPEC_COMMIT}/spec" "$SASS_SPEC_DIR/spec"
        rm -rf "$TMPDIR"
        echo "sass-spec extracted to $SASS_SPEC_DIR/spec"
    fi
fi
