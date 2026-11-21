#!/usr/bin/env bash
# GitCraft E2 — validación automática únicamente en develop.
#
# No publica código, no ejecuta GitHub Actions y no realiza operaciones sobre
# el repositorio de GitCraft. Los tests Rust crean sus propios repositorios
# temporales, con rutas únicas, para verificar stage, unstage y commit local.

set -u
set -o pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
cd "$ROOT" || exit 1

for PROGRAM in git cargo; do
    if ! command -v "$PROGRAM" >/dev/null 2>&1; then
        echo "ERROR: falta $PROGRAM; no se puede validar E2."
        exit 1
    fi
done

if [[ "$(git branch --show-current)" != "develop" ]]; then
    echo "ERROR: E2 se valida exclusivamente sobre develop."
    exit 1
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: hay cambios sin confirmar; no iniciar pruebas."
    exit 1
fi

START_SHA="$(git rev-parse HEAD)"
echo "Validación E2 sobre develop ($START_SHA)"
echo "Las pruebas escriben exclusivamente en carpetas Git temporales."

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

run_check "Rustfmt E2" cargo fmt --all -- --check
run_check "Compilación nativa E2" cargo check --locked
run_check "Tests del motor de acciones E2" \
    cargo test --locked local_git_actions::tests -- --nocapture
run_check "Regresiones E0 E1 E2" cargo test --locked

if cargo clippy --version >/dev/null 2>&1; then
    run_check "Clippy E2 todos los targets" cargo clippy --locked --all-targets
else
    echo "ERROR: Clippy no disponible."
    FAILURES=$((FAILURES + 1))
fi

# Las nuevas funciones nativas están bajo cfg(not(wasm32)).
if command -v rustup >/dev/null 2>&1 \
    && rustup target list --installed | grep -qx "wasm32-unknown-unknown"; then
    run_check "WebAssembly sin operaciones Git locales" \
        cargo check --locked --target wasm32-unknown-unknown
else
    echo "PENDIENTE: WebAssembly (target no instalado)."
fi

if [[ "$(git rev-parse HEAD)" != "$START_SHA" ]]; then
    echo "ERROR: HEAD cambió durante la validación."
    FAILURES=$((FAILURES + 1))
fi
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "ERROR: se modificaron archivos seguidos del repositorio."
    FAILURES=$((FAILURES + 1))
fi

echo
if [[ "$FAILURES" -eq 0 ]]; then
    echo "OK: controles automáticos E2 completados; faltan pruebas manuales."
    exit 0
fi
echo "ERROR: $FAILURES verificaciones de E2 fallaron."
exit 1
