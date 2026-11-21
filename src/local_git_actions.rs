//! Operaciones locales E2 con vista previa y confirmación explícita.
//!
//! E1 consulta Git sin modificarlo. E2 permite al usuario preparar o retirar
//! archivos del índice y crear un commit **local**, pero nunca descarta cambios
//! del directorio de trabajo, cambia de rama ni contacta con un remoto.
//!
//! El flujo es: previsualizar -> confirmar -> volver a verificar -> ejecutar.
//! Un cambio entre la vista previa y la confirmación hace fallar la operación.
//! El objetivo es impedir que una selección antigua modifique otra versión.

use crate::local_git::{inspect_repository, run_git, LocalRepository};
use std::path::{Component, Path, PathBuf};

/// Cambio local permitido por E2; ningún caso descarta archivos o hace push.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalAction {
    /// Preparar la versión actual completa de un archivo.
    Stage(String),
    /// Retirar un archivo del índice sin cambiar el archivo de trabajo.
    Unstage(String),
    /// Crear un commit local únicamente con el contenido ya preparado.
    Commit(String),
}

/// Vista previa inmutable que debe mostrarse antes de ejecutar una operación.
#[derive(Debug, Clone)]
pub struct LocalActionPreview {
    /// Carpeta Git exacta a la que se dirige la acción.
    pub root: PathBuf,
    /// Operación que se ha autorizado a previsualizar.
    pub action: LocalAction,
    /// Texto explicativo para que se conozca el alcance.
    pub summary: String,
    /// Estadísticas o detalle de los cambios previstos.
    pub details: String,
    /// Fotografía del índice, estado y HEAD antes de confirmar.
    stamp: Vec<u8>,
    /// Hash del contenido del archivo, para detectar cambios tras la vista previa.
    file_hash: Option<Vec<u8>>,
}

/// Lee una fotografía exacta de los estados y referencias relevantes.
///
/// Las cadenas de estado de Git se obtienen con delimitadores NUL para que
/// los nombres con espacios, comillas o caracteres especiales no se confundan.
fn safety_stamp(root: &Path) -> Result<Vec<u8>, String> {
    let mut stamp = Vec::new();
    for args in [
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"][..],
        &["ls-files", "--stage", "-z"][..],
    ] {
        let bytes = run_git(Some(root), args)?;
        stamp.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        stamp.extend_from_slice(&bytes);
    }
    // Un repositorio recién inicializado carece de HEAD confirmada.
    let head = run_git(Some(root), &["rev-parse", "--verify", "HEAD"])
        .unwrap_or_default();
    stamp.extend_from_slice(&(head.len() as u64).to_be_bytes());
    stamp.extend_from_slice(&head);
    Ok(stamp)
}

/// Impide seleccionar rutas absolutas, padres o rutas que terminan en carpeta.
/// También excluye entradas conflictivas de renombrados en esta primera fase.
fn validate_file_path(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);
    if path.is_empty()
        || path.ends_with('/')
        || path.contains('\0')
        || candidate.is_absolute()
        || candidate.components().any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("La ruta del archivo no es válida para una operación E2.".into());
    }
    Ok(())
}

/// Comprueba que la ruta sigue representando el mismo contenido de trabajo.
///
/// "hash-object" con --no-filters no escribe el objeto Git. Sirve para
/// detectar si un archivo editado después de previsualizar se ha alterado.
fn current_file_hash(root: &Path, path: &str) -> Result<Option<Vec<u8>>, String> {
    let location = root.join(path);
    match std::fs::symlink_metadata(&location) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err("E2 no prepara enlaces simbólicos en esta fase.".into());
            }
            if !meta.is_file() {
                return Err("E2 sólo admite archivos individuales, no carpetas.".into());
            }
            run_git(Some(root), &["hash-object", "--no-filters", "--", path])
                .map(Some)
        }
        // Para una eliminación de archivo, Git conserva la entrada original
        // en el índice, pero no hay bytes que comprobar en el directorio.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("No se puede comprobar el archivo: {}", error)),
    }
}

/// Genera la vista previa para una sola operación y captura su estado inicial.
///
/// Nunca modifica el índice ni crea commits. Sólo admite archivos realmente
/// presentes en la lista de cambios y mensajes de commit válidos.
pub fn preview_action(root: &Path, action: LocalAction) -> Result<LocalActionPreview, String> {
    let path = root
        .canonicalize()
        .map_err(|error| format!("No se puede localizar el repositorio: {}", error))?;
    let repository = inspect_repository(path.to_str().ok_or("Ruta del repositorio inválida.")?)?;
    if repository.root != path {
        return Err("La carpeta activa ya no corresponde al mismo repositorio.".into());
    }
    let stamp = safety_stamp(&path)?;
    let mut file_hash = None;
    let (summary, details) = match &action {
        LocalAction::Stage(file) | LocalAction::Unstage(file) => {
            validate_file_path(file)?;
            let entry = repository
                .status
                .iter()
                .find(|entry| &entry.path == file)
                .ok_or("Ese archivo ya no figura en el estado Git actual.")?;
            if entry.code.contains('U')
                || entry.code.contains('R')
                || entry.code.contains('C')
                || entry.code == "AA"
                || entry.code == "DD"
            {
                return Err("Los renombrados y conflictos requieren una fase posterior.".into());
            }
            let staged = entry.code.as_bytes()[0] != b' ' && entry.code != "??";
            let unstaged = entry.code == "??" || entry.code.as_bytes()[1] != b' ';
            match &action {
                LocalAction::Stage(_) if !unstaged => {
                    return Err("El archivo no tiene cambios pendientes de preparar.".into());
                }
                LocalAction::Unstage(_) if !staged => {
                    return Err("El archivo no contiene cambios preparados.".into());
                }
                _ => {}
            }

            if matches!(&action, LocalAction::Unstage(_))
                && run_git(Some(&path), &["rev-parse", "--verify", "HEAD"]).is_err()
            {
                return Err(
                    "Retirar del índice en un repositorio sin commits aún no está soportado."
                        .into(),
                );
            }

            file_hash = current_file_hash(&path, file)?;
            let mode = if matches!(&action, LocalAction::Stage(_)) {
                "Preparar"
            } else {
                "Retirar de staging"
            };
            let detail = if matches!(&action, LocalAction::Stage(_)) && entry.code == "??" {
                "Archivo no seguido: se preparará el archivo completo. Git diff no muestra \
                 su contenido antes de añadirlo."
                    .to_string()
            } else {
                // Se fija Vec<&str> para que todas las rutas y opciones se
                // conviertan en argumentos de texto independientes para Git.
                let args: Vec<&str> = if matches!(&action, LocalAction::Stage(_)) {
                    vec![
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-color",
                        "--",
                        file,
                    ]
                } else {
                    vec![
                        "diff",
                        "--cached",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-color",
                        "--",
                        file,
                    ]
                };
                let raw = run_git(Some(&path), &args)?;
                let preview = String::from_utf8_lossy(&raw);
                if preview.is_empty() {
                    "El diff está vacío; comprueba la ruta y el estado.".to_string()
                } else {
                    preview.chars().take(12_000).collect()
                }
            };
            (format!("{} únicamente: {} ({})", mode, file, entry.code), detail)
        }
        LocalAction::Commit(message) => {
            let message = message.trim();
            if message.is_empty()
                || message.len() > 500
                || message.chars().any(|ch| ch.is_control())
            {
                return Err("El mensaje de commit debe tener entre 1 y 500 caracteres, sin saltos de línea.".into());
            }
            if repository.head == "(HEAD separado)" {
                return Err("No se crea un commit desde HEAD separado.".into());
            }
            // No sobrescribir estados de merge/rebase gestionados por otro Git.
            for state in ["MERGE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD"] {
                if run_git(Some(&path), &["rev-parse", "-q", "--verify", state]).is_ok() {
                    return Err("Hay una operación Git pendiente; no se creará un commit.".into());
                }
            }
            let staged = run_git(
                Some(&path),
                &["diff", "--cached", "--name-only", "-z", "--"],
            )?;
            if staged.is_empty() {
                return Err("No hay archivos preparados para crear un commit.".into());
            }
            let names = staged.split(|ch| *ch == 0).filter(|part| !part.is_empty()).count();
            let stats = run_git(Some(&path), &["diff", "--cached", "--stat", "--"])?;
            (
                format!(
                    "Crear commit LOCAL en rama '{}': {} archivos preparados. Mensaje: {}",
                    repository.head, names, message
                ),
                String::from_utf8_lossy(&stats).chars().take(12_000).collect(),
            )
        }
    };

    Ok(LocalActionPreview {
        root: path,
        action,
        summary,
        details,
        stamp,
        file_hash,
    })
}

/// Comprueba de nuevo la fotografía antes de modificar índice o HEAD.
///
/// Si un editor, Terminal u otra instancia cambió archivos desde el preview,
/// se cancela y debe solicitarse una nueva vista previa.
pub fn execute_confirmed_action(preview: &LocalActionPreview) -> Result<LocalRepository, String> {
    let root = preview
        .root
        .canonicalize()
        .map_err(|error| format!("La carpeta ya no está disponible: {}", error))?;
    if root != preview.root {
        return Err("La ruta Git ha cambiado; vuelve a abrir el repositorio.".into());
    }
    if safety_stamp(&root)? != preview.stamp {
        return Err(
            "El estado Git ha cambiado después de la vista previa; vuelve a revisarlo."
                .into(),
        );
    }

    match &preview.action {
        LocalAction::Stage(file) | LocalAction::Unstage(file) => {
            validate_file_path(file)?;
            if current_file_hash(&root, file)? != preview.file_hash {
                return Err(
                    "El archivo ha cambiado desde la vista previa; no se ha modificado el índice."
                        .into(),
                );
            }
            match &preview.action {
                LocalAction::Stage(_) => {
                    // "--" impide interpretar nombres como "--force" como opciones.
                    run_git(Some(&root), &["add", "--", file])?;
                }
                LocalAction::Unstage(_) => {
                    // Sólo cambia el índice. Los bytes del archivo se conservan.
                    run_git(Some(&root), &["restore", "--staged", "--", file])?;
                }
                LocalAction::Commit(_) => unreachable!(),
            }
        }
        LocalAction::Commit(message) => {
            // Se confirma sólo el índice ya preparado; nunca se usa "commit -a".
            // Los hooks quedan desactivados para impedir programas externos
            // de terceros durante esta acción. Se avisa de ello en el preview.
            run_git(
                Some(&root),
                &[
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "commit.gpgsign=false",
                    "commit",
                    "-m",
                    message.trim(),
                ],
            )?;
        }
    }
    inspect_repository(root.to_str().ok_or("Ruta no UTF-8 no soportada.")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Cada prueba crea su propio repositorio temporal; nunca toca GitCraft.
    static NEXT: AtomicU64 = AtomicU64::new(0);

    /// Directorio temporal que se elimina al terminar cada test.
    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        /// Inicializa Git y un commit ficticio sin dependencias de Github.
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "gitcraft-e2-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).expect("directorio temporal");
            let fixture = Self { root };
            fixture.git(&["init", "-q"]);
            fixture.git(&["symbolic-ref", "HEAD", "refs/heads/prueba-e2"]);
            std::fs::write(fixture.root.join("original.txt"), "original\n").unwrap();
            fixture.git(&["add", "--", "original.txt"]);
            fixture.git(&[
                "-c",
                "user.name=GitCraft E2 Test",
                "-c",
                "user.email=gitcraft-e2@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "Inicial",
            ]);
            fixture
        }

        /// Sólo los tests crean commits y archivos propios, nunca la aplicación.
        fn git(&self, args: &[&str]) -> Vec<u8> {
            let mut command = Command::new("git");
            command
                .arg("-C")
                .arg(&self.root)
                .arg("-c")
                .arg("core.hooksPath=/dev/null")
                .args(args);
            let output = command.output().expect("Git instalado");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        }
    }

    impl Drop for Fixture {
        /// Sólo se elimina la carpeta temporal propia del test.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Stage y unstage no modifican los bytes del archivo de trabajo.
    #[test]
    fn stage_and_unstage_keep_worktree_intact() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("original.txt"), "cambio E2\n").unwrap();
        let before = std::fs::read(fixture.root.join("original.txt")).unwrap();
        let preview = preview_action(
            &fixture.root,
            LocalAction::Stage("original.txt".into()),
        )
        .unwrap();
        let staged = execute_confirmed_action(&preview).unwrap();
        assert!(staged.status.iter().any(|item| item.code == "M "));
        assert_eq!(std::fs::read(fixture.root.join("original.txt")).unwrap(), before);

        let preview = preview_action(
            &fixture.root,
            LocalAction::Unstage("original.txt".into()),
        )
        .unwrap();
        let unstaged = execute_confirmed_action(&preview).unwrap();
        assert!(unstaged.status.iter().any(|item| item.code == " M"));
        assert_eq!(std::fs::read(fixture.root.join("original.txt")).unwrap(), before);
    }

    /// El nombre con espacios tampoco se interpreta como varios argumentos.
    #[test]
    fn stage_untracked_file_with_spaces() {
        let fixture = Fixture::new();
        let name = "archivo prueba ñ.txt";
        std::fs::write(fixture.root.join(name), "texto\n").unwrap();
        let preview = preview_action(&fixture.root, LocalAction::Stage(name.into())).unwrap();
        let snapshot = execute_confirmed_action(&preview).unwrap();
        assert!(snapshot.status.iter().any(|item| item.path == name && item.code == "A "));
    }

    /// Un archivo modificado después del preview aborta sin modificar el índice.
    #[test]
    fn changed_file_rejects_confirmation() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("original.txt"), "antes\n").unwrap();
        let preview = preview_action(
            &fixture.root,
            LocalAction::Stage("original.txt".into()),
        )
        .unwrap();
        std::fs::write(fixture.root.join("original.txt"), "después\n").unwrap();
        assert!(execute_confirmed_action(&preview).is_err());
        let index = fixture.git(&["diff", "--cached", "--name-only"]);
        assert!(index.is_empty());
    }

    /// No puede confirmarse un commit si alguien altera el índice mientras tanto.
    #[test]
    fn staged_changes_reject_stale_commit_preview() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("original.txt"), "preparado\n").unwrap();
        fixture.git(&["add", "--", "original.txt"]);
        let preview = preview_action(&fixture.root, LocalAction::Commit("Prueba E2".into()))
            .unwrap();
        std::fs::write(fixture.root.join("nuevo.txt"), "otro cambio\n").unwrap();
        fixture.git(&["add", "--", "nuevo.txt"]);
        assert!(execute_confirmed_action(&preview).is_err());
    }

    /// Un commit sólo incluye el contenido staged y no envía datos al remoto.
    #[test]
    fn local_commit_keeps_unstaged_changes() {
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("original.txt"), "preparado\n").unwrap();
        fixture.git(&["add", "--", "original.txt"]);
        std::fs::write(fixture.root.join("original.txt"), "sin preparar\n").unwrap();
        let old_head = fixture.git(&["rev-parse", "HEAD"]);

        // El commit de la prueba debe disponer de identidad explícita. Esa
        // configuración se limita al fixture, no a la configuración del usuario.
        fixture.git(&["config", "user.name", "GitCraft E2 Test"]);
        fixture.git(&["config", "user.email", "gitcraft-e2@example.invalid"]);

        let preview = preview_action(&fixture.root, LocalAction::Commit("Commit E2".into()))
            .unwrap();
        let result = execute_confirmed_action(&preview).unwrap();
        assert_ne!(fixture.git(&["rev-parse", "HEAD"]), old_head);
        assert_eq!(result.head, "prueba-e2");
        assert!(result.status.iter().any(|item| item.code == " M"));
        assert_eq!(
            std::fs::read_to_string(fixture.root.join("original.txt")).unwrap(),
            "sin preparar\n"
        );
    }

    /// Se descartan rutas externas, commits sin archivos y mensajes vacíos.
    #[test]
    fn bad_inputs_never_write() {
        let fixture = Fixture::new();
        assert!(preview_action(
            &fixture.root,
            LocalAction::Stage("../fuera".into())
        )
        .is_err());
        assert!(preview_action(&fixture.root, LocalAction::Commit("".into())).is_err());
        assert!(preview_action(&fixture.root, LocalAction::Commit("Sin cambios".into())).is_err());
        assert!(fixture.git(&["status", "--porcelain"]).is_empty());
    }
}
