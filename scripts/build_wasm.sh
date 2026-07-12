#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

TARGET="wasm32v1-none"
PROFILE="release"
PACKAGE=""
OPTIMIZE="true"

print_usage() {
  cat <<'EOF'
Usage: ./scripts/build_wasm.sh [options]

Options:
  --target <target>      WASM target triple (default: wasm32v1-none)
  --profile <profile>    Cargo profile to build (default: release)
  --package <name>       Build only the named package
  --no-optimize          Skip wasm-opt post-processing
  --help                 Show this help message
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --target)
      shift
      TARGET="$1"
      shift
      ;;
    --profile)
      shift
      PROFILE="$1"
      shift
      ;;
    --package|--contract)
      shift
      PACKAGE="$1"
      shift
      ;;
    --no-optimize)
      OPTIMIZE="false"
      shift
      ;;
    --help)
      print_usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      print_usage
      exit 1
      ;;
  esac
done

export RUSTFLAGS="${RUSTFLAGS:-"-C link-arg=-s"}"

BUILD_CMD=(cargo build --workspace --profile "$PROFILE" --target "$TARGET")
if [ -n "$PACKAGE" ]; then
  BUILD_CMD+=(--package "$PACKAGE")
fi

printf 'Building optimized WASM: target=%s profile=%s package=%s\n' "$TARGET" "$PROFILE" "${PACKAGE:-all workspace packages}"
printf 'Using RUSTFLAGS=%s\n' "$RUSTFLAGS"

"${BUILD_CMD[@]}"

ARTIFACT_DIR="$ROOT_DIR/target/$TARGET/$PROFILE"
if [ ! -d "$ARTIFACT_DIR" ]; then
  echo "Build completed, but artifact directory does not exist: $ARTIFACT_DIR" >&2
  exit 1
fi

find "$ARTIFACT_DIR" -maxdepth 1 -type f -name '*.wasm' -print | sort

if [ "$OPTIMIZE" = "true" ] && command -v wasm-opt >/dev/null 2>&1; then
  echo "Found wasm-opt, optimizing generated artifacts..."
  while read -r wasm_file; do
    opt_file="${wasm_file%.wasm}.opt.wasm"
    wasm-opt -Oz -o "$opt_file" "$wasm_file"
    printf 'Optimized %s -> %s\n' "$wasm_file" "$opt_file"
  done < <(find "$ARTIFACT_DIR" -maxdepth 1 -type f -name '*.wasm' | sort)
else
  if [ "$OPTIMIZE" = "true" ]; then
    echo "wasm-opt not available; skipping post-build wasm optimization."
  fi
fi

printf '\nProduction WASM build complete. Artifacts in %s\n' "$ARTIFACT_DIR"
