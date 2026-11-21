#!/usr/bin/env bash
# Verificación local del rediseño de interfaz tipo SourceTree.
#
# No ejecuta Actions ni modifica ramas, referencias o archivos del
# repositorio real. Rustfmt se consulta únicamente con --check.

set -u
set -o pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
cd "$ROOT" || exit 1

for program in git cargo; do
    if ! command -v "$program" >/dev/null 2>&1; then
        echo "ERROR: falta $program."
        exit 1
    fi
done

if [[ "$(git branch --show-current)" != "develop" ]]; then
    echo "ERROR: sólo se valida en la rama develop."
    exit 1
fi

if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "ERROR: existen cambios o archivos no seguidos. Revisar antes de validar."
    exit 1
fi

HEAD_BEFORE="$(git rev-parse HEAD)"
echo "Validación de rediseño UX en $HEAD_BEFORE"
echo "No se modifica ningún repositorio real."

FAILURES=0
run_check() {
    local title="$1"
    shift
    echo
    echo "===== $title ====="
    if "$@"; then
        echo "OK: $title"
    else
        echo "ERROR: $title"
        FAILURES=$((FAILURES + 1))
    fi
}

run_check "Formato Rustfmt" cargo fmt --all -- --check
run_check "Compilación escritorio" cargo check --locked
run_check "Tests de navegación y archivos" \
    cargo test --locked ui_ -- --nocapture
run_check "Regresiones completas E0-E3-B" cargo test --locked
if cargo clippy --version >/dev/null 2>&1; then
    run_check "Clippy" cargo clippy --locked --all-targets
else
    echo "ERROR: Clippy no está disponible."
    FAILURES=$((FAILURES + 1))
fi

if command -v rustup >/dev/null 2>&1 \
    && rustup target list --installed | grep -qx wasm32-unknown-unknown; then
    run_check "WebAssembly" cargo check --locked --target wasm32-unknown-unknown
else
    echo "PENDIENTE: target WebAssembly no instalado; no se infiere validación web."
fi

if [[ "$(git rev-parse HEAD)" != "$HEAD_BEFORE" ]]; then
    echo "ERROR: HEAD cambió durante las pruebas."
    FAILURES=$((FAILURES + 1))
fi

if [[ -n "$(git status --porcelain --untracked-files=all)" ]]; then
    echo "ERROR: aparecieron cambios en el repositorio GitCraft."
    FAILURES=$((FAILURES + 1))
fi

echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "OK: controles automáticos UX superados; pruebas visuales pendientes."
    exit 0
fi

echo "ERROR: $FAILURES verificaciones de UX fallaron."
exit 1
