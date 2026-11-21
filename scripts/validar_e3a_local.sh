#!/usr/bin/env bash
# GitCraft E3-A: grafo, compare, blame, reflog, tags y stash de solo lectura.
#
# Sólo comprueba develop y no cambia ninguna rama. Las pruebas Rust E3-A
# preparan datos propios dentro del directorio temporal del sistema.
# Este script no publica código ni ejecuta GitHub Actions.

set -u
set -o pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
cd "$ROOT" || exit 1

for PROGRAM in git cargo; do
    if ! command -v "$PROGRAM" >/dev/null 2>&1; then
        echo "ERROR: no está instalado $PROGRAM."
        exit 1
    fi
done

if [[ "$(git branch --show-current)" != "develop" ]]; then
    echo "ERROR: las pruebas E3-A sólo se ejecutan en develop."
    exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: existen cambios locales sin confirmar; revisarlos primero."
    exit 1
fi

BEFORE="$(git rev-parse HEAD)"
echo "Validación E3-A del commit $BEFORE"
echo "Sólo se crean repositorios temporales en las pruebas."

FAILURES=0
run_check() {
    local NAME="$1"
    shift
    echo
    echo "===== $NAME ====="
    if "$@"; then
        echo "OK: $NAME"
    else
        echo "ERROR: $NAME"
        FAILURES=$((FAILURES + 1))
    fi
}

# El formato nunca se aplica automáticamente sobre el árbol de trabajo.
run_check "Rustfmt E3-A" cargo fmt --all -- --check
run_check "Compilación E3-A" cargo check --locked
run_check "Tests de historial E3-A" \
    cargo test --locked local_git_history::tests -- --nocapture
run_check "Regresiones E0 E1 E2 y E3-A" cargo test --locked
if cargo clippy --version >/dev/null 2>&1; then
    run_check "Clippy E3-A" cargo clippy --locked --all-targets
else
    echo "ERROR: Clippy no está disponible."
    FAILURES=$((FAILURES + 1))
fi

# WASM conserva la pantalla informativa: Git local nativo no se compila.
if command -v rustup >/dev/null 2>&1 \
    && rustup target list --installed | grep -qx wasm32-unknown-unknown; then
    run_check "Compilación WebAssembly" \
        cargo check --locked --target wasm32-unknown-unknown
else
    echo "PENDIENTE: no hay target wasm32-unknown-unknown instalado."
fi

# Verificar que el script no ha alterado HEAD ni los archivos seguidos.
if [[ "$(git rev-parse HEAD)" != "$BEFORE" ]]; then
    echo "ERROR: HEAD cambió durante la validación."
    FAILURES=$((FAILURES + 1))
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: existen archivos seguidos modificados tras la validación."
    FAILURES=$((FAILURES + 1))
fi

echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "OK: controles automáticos E3-A completados; faltan pruebas visuales."
    exit 0
fi
echo "ERROR: $FAILURES verificaciones E3-A fallaron."
exit 1
