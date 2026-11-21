#!/bin/sh
# Verifica el formato Rust de todos los módulos de GitCraft sin escribir.
#
# Es invocado por los hooks locales o se puede ejecutar a mano. Su finalidad
# es impedir publicar archivos con diferencias de rustfmt. No ejecuta
# git add, commit, push, reset ni cambia de rama.
set -eu

ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || {
    echo "ERROR: no estamos dentro de un repositorio Git." >&2
    exit 1
}
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo/Rustfmt no está disponible. No publicar código Rust." >&2
    exit 1
fi

if ! cargo fmt --all -- --check; then
    echo "ERROR: el formato Rust no es válido. El autor debe ejecutar primero" >&2
    echo "       bash scripts/preparar_rust_para_entrega.sh" >&2
    echo "No se ha cambiado ningún archivo desde este control." >&2
    exit 1
fi

echo "OK: formato Rust comprobado antes de publicar."
