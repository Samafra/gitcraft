//! E3-B: creación confirmada de ramas y etiquetas locales sin cambiar archivos.
//!
//! Este módulo añade sólo dos operaciones de escritura avanzadas a GitCraft:
//! crear una referencia de rama o una etiqueta ligera apuntando a un commit.
//! Nunca cambia HEAD, ejecuta checkout, borra referencias ni contacta con red.
//!
//! Cada acción sigue dos pasos: preparar una vista previa y confirmar después.
//! Entre ambos pasos se comprueba si cambiaron HEAD, el índice, el directorio
//! de trabajo o el conjunto de ramas y etiquetas. Si algo cambió, se cancela.
//!
//! Las funciones usan el ejecutor Git de E1, que pasa argumentos individuales
//! al proceso. No se interpretan comandos escritos en la interfaz.

use crate::local_git::{inspect_repository, run_git};
use std::path::{Path, PathBuf};

/// Tipo de referencia nueva que se permite crear en E3-B.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    /// Una rama local; la aplicación no cambia a ella después de crearla.
    Branch,
    /// Una etiqueta ligera, sin firma ni mensaje de anotación.
    Tag,
}

impl Default for RefKind {
    /// La opción inicial es una rama; no se ejecuta nada al seleccionarla.
    fn default() -> Self {
        Self::Branch
    }
}

/// Petición de crear una referencia a un commit existente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefCreation {
    /// Rama local o etiqueta ligera según la variante seleccionada.
    pub kind: RefKind,
    /// Nombre nuevo proporcionado por la persona usuaria.
    pub name: String,
    /// Hash completo de un commit que ya está en el repositorio local.
    pub target: String,
}

/// Resumen revisable de una operación todavía no ejecutada.
#[derive(Debug, Clone)]
pub struct RefPreview {
    /// Raíz del repositorio resuelta antes de iniciar la operación.
    pub root: PathBuf,
    /// Todos los parámetros exactos que se confirmarán más adelante.
    pub action: RefCreation,
    /// Mensaje visible con nombre, tipo y SHA de destino.
    pub summary: String,
    /// Fotografía del repositorio; no se serializa en preferencias.
    stamp: Vec<u8>,
}

/// Resultado explícito: Git terminó de crear la referencia indicada.
///
/// La actualización de la interfaz se hace después, por separado, de forma
/// que un fallo de recarga no se confunda con un fallo de escritura Git.
#[derive(Debug, Clone)]
pub struct RefCreated {
    /// Repositorio en el que se creó la referencia.
    pub root: PathBuf,
    /// Referencia creada: se utiliza únicamente para el aviso de éxito.
    pub action: RefCreation,
}

/// Obtiene una raíz de Git ya abierta y rechaza rutas de otra carpeta.
///
/// La aplicación de escritorio puede abrir subcarpetas al principio, pero
/// las acciones reciben la ruta exacta y normalizada de LocalRepository.
fn checked_root(root: &Path) -> Result<PathBuf, String> {
    let canonical = root
        .canonicalize()
        .map_err(|error| format!("No se puede localizar el repositorio: {}", error))?;
    let text = canonical
        .to_str()
        .ok_or("La ruta Git no se puede convertir a texto UTF-8.")?;
    let found = inspect_repository(text)?;
    if found.root != canonical {
        return Err("El repositorio actual ya no coincide con el solicitado.".into());
    }
    Ok(canonical)
}

/// Devuelve el prefijo Git que corresponde al tipo de referencia.
///
/// Sólo se admiten ramas locales y etiquetas. No se manipulan referencias
/// remotas ni los nombres especiales de HEAD y stash.
fn reference_path(action: &RefCreation) -> String {
    match action.kind {
        RefKind::Branch => format!("refs/heads/{}", action.name),
        RefKind::Tag => format!("refs/tags/{}", action.name),
    }
}

/// Rechaza opciones de línea de comandos y nombres de referencias inválidos.
///
/// Git realiza la validación final con check-ref-format. No se confía sólo
/// en que el cuadro de texto tenga un aspecto correcto.
fn validate_name(root: &Path, action: &RefCreation) -> Result<(), String> {
    let name = action.name.as_str();
    if name.is_empty()
        || name.len() > 120
        || name.starts_with('-')
        || name.contains('\0')
        || name.chars().any(char::is_control)
        || name.trim() != name
    {
        return Err("El nombre de la rama o etiqueta no es válido.".into());
    }
    let result = match action.kind {
        RefKind::Branch => run_git(Some(root), &["check-ref-format", "--branch", name]),
        RefKind::Tag => {
            let full = reference_path(action);
            run_git(Some(root), &["check-ref-format", &full])
        }
    };
    result.map_err(|_| "Git ha rechazado el nombre de la nueva referencia.".to_string())?;
    Ok(())
}

/// Verifica que el destino es el hash completo de un objeto commit.
///
/// Nunca se pasan revisiones libres como HEAD~3 o expresiones de consola.
/// Los SHA proceden del selector de commits existente en E3-A.
fn validate_target(root: &Path, sha: &str) -> Result<(), String> {
    if !(sha.len() == 40 || sha.len() == 64)
        || !sha.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err("Selecciona un SHA completo de un commit válido.".into());
    }
    let kind = run_git(Some(root), &["cat-file", "-t", sha])?;
    if String::from_utf8_lossy(&kind).trim() != "commit" {
        return Err("El destino no es un objeto commit.".into());
    }
    Ok(())
}

/// Evita sobrescribir cualquier rama o etiqueta existente.
///
/// Git branch/tag sin -f también rechaza duplicados; esta lectura anticipada
/// sirve para explicarlo antes de ofrecer el botón de confirmación.
fn ensure_ref_absent(root: &Path, action: &RefCreation) -> Result<(), String> {
    let full = reference_path(action);
    let names = run_git(
        Some(root),
        &["for-each-ref", "--format=%(refname)", "--", &full],
    )?;
    if String::from_utf8_lossy(&names)
        .lines()
        .any(|line| line == full)
    {
        return Err("Ya existe una rama o etiqueta con ese nombre.".into());
    }
    Ok(())
}

/// Añade la longitud de un resultado antes de sus bytes, sin ambigüedades.
///
/// El formato es interno: sólo se compara la igualdad exacta de dos
/// instantáneas; no se guarda ni se comparte con servidores.
fn append_section(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    out.extend_from_slice(bytes);
}

/// Construye una fotografía para detectar cambios mientras se confirma.
///
/// Incluye archivos modificados, contenido del índice, ramas, etiquetas,
/// SHA de HEAD y el nombre simbólico de HEAD. Dos ramas distintas podrían
/// apuntar al mismo commit, por eso se comprueban los dos últimos campos.
fn safety_stamp(root: &Path) -> Result<Vec<u8>, String> {
    let mut stamp = Vec::new();
    for args in [
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"][..],
        &["ls-files", "--stage", "-z"][..],
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            "refs/heads",
            "refs/tags",
        ][..],
    ] {
        let result = run_git(Some(root), args)?;
        append_section(&mut stamp, &result);
    }
    let head = run_git(Some(root), &["rev-parse", "--verify", "HEAD"])?;
    append_section(&mut stamp, &head);
    let branch = run_git(Some(root), &["symbolic-ref", "-q", "--short", "HEAD"])
        .unwrap_or_default();
    append_section(&mut stamp, &branch);
    Ok(stamp)
}

/// Prepara una creación de referencia, sin escribir nada en Git.
///
/// Devuelve Ok(vista_previa) sólo cuando el nombre no existe y el SHA apunta
/// a un commit válido. Err(mensaje) indica que no se puede continuar.
pub fn preview_ref(root: &Path, action: RefCreation) -> Result<RefPreview, String> {
    let root = checked_root(root)?;
    validate_name(&root, &action)?;
    validate_target(&root, &action.target)?;
    ensure_ref_absent(&root, &action)?;
    let stamp = safety_stamp(&root)?;
    let kind = match action.kind {
        RefKind::Branch => "rama local (sin cambiar a ella)",
        RefKind::Tag => "etiqueta ligera (sin mover HEAD)",
    };
    let summary = format!(
        "Crear {}: {}. Commit de destino: {}. No se modifican archivos ni se hace push.",
        kind, action.name, action.target
    );
    Ok(RefPreview {
        root,
        action,
        summary,
        stamp,
    })
}

/// Confirma una vista previa tras verificar otra vez el estado Git.
///
/// Si el estado o cualquier referencia cambió, devuelve Err sin ejecutar
/// la creación. El comando final no usa opciones de sobrescritura ni push.
pub fn execute_ref(preview: &RefPreview) -> Result<RefCreated, String> {
    let root = checked_root(&preview.root)?;
    if root != preview.root {
        return Err("La ruta Git cambió; vuelve a revisar la operación.".into());
    }
    validate_name(&root, &preview.action)?;
    validate_target(&root, &preview.action.target)?;
    if safety_stamp(&root)? != preview.stamp {
        return Err(
            "Git ha cambiado después de la vista previa; revisa de nuevo.".into(),
        );
    }
    ensure_ref_absent(&root, &preview.action)?;

    // Nunca se incluye -f, --force, -D o checkout. Git protege también
    // el caso de que otro proceso cree el mismo nombre inmediatamente antes.
    // Se deshabilitan hooks exclusivamente en este subproceso: una operación
    // de referencia no debe ejecutar scripts externos del repositorio.
    // Igual que E2, esto significa que NO se ejecutan validaciones de hooks.
    match preview.action.kind {
        RefKind::Branch => {
            run_git(
                Some(&root),
                &[
                    "-c",
                    "core.hooksPath=/dev/null",
                    "branch",
                    &preview.action.name,
                    &preview.action.target,
                ],
            )?;
        }
        RefKind::Tag => {
            run_git(
                Some(&root),
                &[
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "tag.gpgSign=false",
                    "tag",
                    &preview.action.name,
                    &preview.action.target,
                ],
            )?;
        }
    }
    Ok(RefCreated {
        root,
        action: preview.action.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Cada test construye su propio directorio y no toca Samafra/gitcraft.
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    /// Carpeta Git de prueba, eliminada únicamente al terminar su test.
    struct Fixture {
        root: PathBuf,
        initial_head: String,
    }

    impl Fixture {
        /// Crea dos commits locales para seleccionar un destino real.
        fn new() -> Self {
            let suffix = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "gitcraft-e3b-{}-{}-{}",
                std::process::id(),
                suffix,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("reloj")
                    .as_nanos()
            ));
            std::fs::create_dir(&root).expect("crear fixture");
            let mut fixture = Self {
                root,
                initial_head: String::new(),
            };
            fixture.git(&["init", "-q"]);
            fixture.git(&["symbolic-ref", "HEAD", "refs/heads/test-e3b"]);
            fixture.git(&["config", "user.name", "GitCraft E3B Test"]);
            fixture.git(&["config", "user.email", "gitcraft-e3b@example.invalid"]);
            std::fs::write(fixture.root.join("ejemplo.txt"), "primero\n").unwrap();
            fixture.git(&["add", "--", "ejemplo.txt"]);
            fixture.git(&["commit", "-qm", "Inicial"]);
            fixture.initial_head = fixture.sha();
            fixture
        }

        /// Ejecuta Git sólo en la carpeta temporal de este caso de prueba.
        fn git(&self, args: &[&str]) -> Vec<u8> {
            let output = Command::new("git")
                .arg("-C")
                .arg(&self.root)
                .arg("-c")
                .arg("core.hooksPath=/dev/null")
                .arg("-c")
                .arg("commit.gpgsign=false")
                .arg("-c")
                .arg("tag.gpgSign=false")
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_INDEX_FILE")
                .env_remove("GIT_WORK_TREE")
                .env("GIT_TERMINAL_PROMPT", "0")
                .output()
                .expect("ejecutar Git en fixture");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        }

        /// Devuelve el SHA completo de HEAD del repositorio de prueba.
        fn sha(&self) -> String {
            String::from_utf8(self.git(&["rev-parse", "HEAD"]))
                .unwrap()
                .trim()
                .to_string()
        }

        /// Genera una petición sin confirmarla todavía.
        fn action(&self, kind: RefKind, name: &str) -> RefCreation {
            RefCreation {
                kind,
                name: name.to_string(),
                target: self.initial_head.clone(),
            }
        }
    }

    impl Drop for Fixture {
        /// Sólo se elimina el directorio creado por el propio test.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Crear una rama local no realiza checkout ni modifica el árbol.
    #[test]
    fn branch_creation_preserves_head_and_files() {
        let fixture = Fixture::new();
        let original = std::fs::read(fixture.root.join("ejemplo.txt")).unwrap();
        let preview = preview_ref(
            &fixture.root,
            fixture.action(RefKind::Branch, "funcionalidad/e3b"),
        )
        .unwrap();
        assert!(fixture.git(&["branch", "--list", "funcionalidad/e3b"]).is_empty());
        execute_ref(&preview).unwrap();
        assert_eq!(fixture.sha(), fixture.initial_head);
        assert_eq!(std::fs::read(fixture.root.join("ejemplo.txt")).unwrap(), original);
        assert_eq!(
            String::from_utf8(fixture.git(&["branch", "--show-current"]))
                .unwrap()
                .trim(),
            "test-e3b"
        );
        assert!(!fixture.git(&["branch", "--list", "funcionalidad/e3b"]).is_empty());
    }

    /// Una etiqueta ligera nueva señala el SHA seleccionado sin mover HEAD.
    #[test]
    fn lightweight_tag_creation_preserves_head() {
        let fixture = Fixture::new();
        let preview =
            preview_ref(&fixture.root, fixture.action(RefKind::Tag, "e3b-prueba")).unwrap();
        execute_ref(&preview).unwrap();
        let tag = String::from_utf8(fixture.git(&["rev-parse", "refs/tags/e3b-prueba"]))
            .unwrap();
        assert_eq!(tag.trim(), fixture.initial_head);
        assert_eq!(fixture.sha(), fixture.initial_head);
        assert!(fixture.git(&["status", "--porcelain"]).is_empty());
    }

    /// No se reemplazan referencias existentes, aunque se repita el clic.
    #[test]
    fn duplicate_names_are_rejected_before_write() {
        let fixture = Fixture::new();
        let action = fixture.action(RefKind::Tag, "duplicada");
        let preview = preview_ref(&fixture.root, action.clone()).unwrap();
        execute_ref(&preview).unwrap();
        assert!(preview_ref(&fixture.root, action).is_err());
        assert!(execute_ref(&preview).is_err());
    }

    /// Las opciones y los nombres Git incorrectos no llegan a branch/tag.
    #[test]
    fn invalid_names_are_rejected() {
        let fixture = Fixture::new();
        for name in ["-f", "../fuera", "nombre con espacios", "mal..nombre", ""] {
            assert!(preview_ref(
                &fixture.root,
                fixture.action(RefKind::Branch, name)
            )
            .is_err());
        }
        assert!(fixture.git(&["status", "--porcelain"]).is_empty());
    }

    /// Un SHA inventado no puede convertirse en nombre de opción Git.
    #[test]
    fn invalid_commit_targets_are_rejected() {
        let fixture = Fixture::new();
        let mut action = fixture.action(RefKind::Branch, "segura");
        action.target = "--force".into();
        assert!(preview_ref(&fixture.root, action).is_err());
    }

    /// Si HEAD cambia antes de confirmar, la referencia no se crea.
    #[test]
    fn changing_head_rejects_stale_preview() {
        let fixture = Fixture::new();
        let preview =
            preview_ref(&fixture.root, fixture.action(RefKind::Branch, "posterior")).unwrap();
        fixture.git(&["symbolic-ref", "HEAD", "refs/heads/otra"]);
        assert!(execute_ref(&preview).is_err());
        assert!(fixture.git(&["branch", "--list", "posterior"]).is_empty());
    }

    /// Un archivo modificado después de la vista previa invalida la acción.
    #[test]
    fn changed_worktree_rejects_stale_preview() {
        let fixture = Fixture::new();
        let preview =
            preview_ref(&fixture.root, fixture.action(RefKind::Tag, "sin-cambio")).unwrap();
        std::fs::write(fixture.root.join("ejemplo.txt"), "cambio posterior\n").unwrap();
        assert!(execute_ref(&preview).is_err());
        assert!(fixture.git(&["tag", "--list", "sin-cambio"]).is_empty());
    }

    /// La creación no debe ejecutar hooks externos configurados por el repo.
    ///
    /// El script ficticio sólo escribiría un archivo dentro del fixture. Si
    /// el motor deja de deshabilitar los hooks, el test lo detecta sin tocar
    /// ningún repositorio real ni iniciar conexiones de red.
    #[cfg(unix)]
    #[test]
    fn creating_branch_does_not_execute_custom_hooks() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new();
        let hooks = fixture.root.join("hooks-e3b");
        std::fs::create_dir(&hooks).unwrap();
        let script = hooks.join("reference-transaction");
        std::fs::write(
            &script,
            "#!/bin/sh\nprintf 'ejecutado' > hook-ejecutado.txt\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&script, permissions).unwrap();
        let hooks_path = hooks.to_str().unwrap();
        fixture.git(&["config", "core.hooksPath", hooks_path]);

        let preview = preview_ref(
            &fixture.root,
            fixture.action(RefKind::Branch, "sin-hooks"),
        )
        .unwrap();
        execute_ref(&preview).unwrap();
        assert!(!fixture.root.join("hook-ejecutado.txt").exists());
    }

    /// Crear otra referencia mientras se revisa invalida la confirmación.
    #[test]
    fn changed_refs_reject_stale_preview() {
        let fixture = Fixture::new();
        let preview =
            preview_ref(&fixture.root, fixture.action(RefKind::Tag, "no-confirmada")).unwrap();
        fixture.git(&["tag", "creada-externamente"]);
        assert!(execute_ref(&preview).is_err());
        assert!(fixture.git(&["tag", "--list", "no-confirmada"]).is_empty());
    }
}
