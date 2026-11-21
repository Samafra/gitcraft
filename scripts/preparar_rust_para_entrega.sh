#!/bin/sh
# Prepara una entrega Rust EN LA MÁQUINA DEL AUTOR, antes de publicarla.
#
# Formatea los archivos Rust de esta copia local y verifica que compilan y
# pasan pruebas. NO hace git add, commit, push, reset ni publica nada.
# Ejecutarlo supone autorizar explícitamente cambios de formato en .rs.
set -eu

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo "ERROR: directorio sin repositorio Git." >&2
    exit 1
}
cd "$ROOT"

if [ "$(git branch --show-current)" != "develop" ]; then
    echo "ERROR: preparar código sólo desde develop, nunca desde main." >&2
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: falta cargo (y Rustfmt); no se publicará la entrega." >&2
    exit 1
fi

echo "== Formato automático antes de publicar =="
cargo fmt --all

echo "== Verificación de formato =="
cargo fmt --all -- --check

echo "== Compilación nativa =="
cargo check --locked

echo "== Pruebas completas =="
cargo test --locked

echo "== Clippy =="
cargo clippy --locked --all-targets

echo "== Estado final; revisar las diferencias antes de confirmar =="
git status --short
git diff --stat

echo "OK: código preparado y verificado LOCALMENTE. No se ha hecho commit ni push."
