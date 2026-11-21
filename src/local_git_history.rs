//! E3-A: herramientas avanzadas de historial Git, exclusivamente de lectura.
//!
//! Este módulo complementa el estado de trabajo E1 y las acciones confirmadas
//! E2. Lee el grafo que genera Git, comparaciones entre dos commits, anotaciones
//! de autoría (blame), reflog, etiquetas y la lista de entradas del stash.
//!
//! Ninguna función hace reset, checkout, merge, rebase, stash pop ni push.
//! Los argumentos se envían como valores independientes al Git instalado;
//! no se usa un intérprete de comandos ni se contacta con la red.

use crate::local_git::{inspect_repository, run_git};
use std::path::{Component, Path, PathBuf};

/// Número máximo de commits que se muestra en el historial avanzado.
const MAX_HISTORY_COMMITS: usize = 80;
/// Número máximo de archivos seleccionables para consultar blame.
const MAX_TRACKED_FILES: usize = 500;
/// Tamaño máximo de texto devuelto a la interfaz para evitar bloqueos.
const MAX_RESULT_CHARS: usize = 120_000;

/// Identifica un commit que puede seleccionarse como extremo de una comparación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRevision {
    /// Hash completo; no se construye a partir de una etiqueta arbitraria.
    pub sha: String,
    /// Hash corto para mostrar junto al mensaje.
    pub short_sha: String,
    /// Primera línea del mensaje de Git.
    pub subject: String,
}

/// Instantánea en memoria: se obtiene sin modificar el repositorio.
#[derive(Debug, Clone)]
pub struct GitHistoryOverview {
    /// Ruta normalizada de la carpeta raíz Git, no una ruta escrita en la UI.
    pub root: PathBuf,
    /// Grafo ASCII dibujado por Git, que respeta padres y uniones.
    pub graph: String,
    /// Commits elegibles para comparar sin introducir revisiones libres.
    pub revisions: Vec<HistoryRevision>,
    /// Cambios registrados por HEAD reflog, hasta los últimos 40.
    pub reflog: Vec<String>,
    /// Nombres de etiquetas locales, sin editar sus referencias.
    pub tags: Vec<String>,
    /// Entradas guardadas en stash, sin aplicarlas ni eliminarlas.
    pub stash: Vec<String>,
    /// Archivos seguidos que admiten blame.
    pub tracked_files: Vec<String>,
    /// Indica si el listado de archivos se ha limitado a 500 entradas.
    pub tracked_truncated: bool,
}

/// Recorta textos grandes para que egui no dibuje respuestas inmensas.
///
/// El proceso Git puede producir una salida mayor que el límite antes de
/// recortarse: esta medida protege principalmente la interfaz gráfica.
fn trim_result(bytes: &[u8]) -> String {
    let text = String::from_utf8_lossy(bytes);
    let mut output: String = text.chars().take(MAX_RESULT_CHARS).collect();
    if text.chars().count() > MAX_RESULT_CHARS {
        output.push_str("\n\n[Salida truncada por GitCraft]");
    }
    output
}

/// Convierte las líneas de git log a revisiones fiables, separadas por NUL.
///
/// Git incluye el hash completo, el hash corto y el asunto de cada commit.
fn parse_revisions(bytes: &[u8]) -> Result<Vec<HistoryRevision>, String> {
    let mut revisions = Vec::new();
    for line in bytes.split(|byte| *byte == b'\n').filter(|line| !line.is_empty()) {
        let parts: Vec<&[u8]> = line.split(|byte| *byte == 0).collect();
        if parts.len() != 3 {
            return Err("El historial Git tiene un formato inesperado.".into());
        }
        let string = |index: usize| String::from_utf8_lossy(parts[index]).into_owned();
        revisions.push(HistoryRevision {
            sha: string(0),
            short_sha: string(1),
            subject: string(2),
        });
    }
    Ok(revisions)
}

/// Convierte un listado NUL de rutas en nombres de archivo visibles.
///
/// Git usa NUL para que las rutas con espacios y saltos de línea se distingan.
fn parse_paths(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).into_owned())
        .collect()
}

/// Lee un listado de líneas, sin interpretar su texto como instrucciones.
fn parse_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

/// Comprueba que el repositorio sigue siendo exactamente la carpeta abierta.
///
/// Las subcarpetas son válidas en E1 al abrirse, pero los servicios posteriores
/// utilizan la raíz ya normalizada para evitar mezclar repositorios.
fn checked_root(root: &Path) -> Result<PathBuf, String> {
    let normalized = root
        .canonicalize()
        .map_err(|error| format!("No se puede abrir el repositorio: {}", error))?;
    let input = normalized
        .to_str()
        .ok_or("E3 requiere una ruta de repositorio válida en UTF-8.")?;
    let snapshot = inspect_repository(input)?;
    if snapshot.root != normalized {
        return Err("La ruta ya no coincide con el repositorio cargado.".into());
    }
    Ok(normalized)
}

/// Genera el resumen de historial avanzado E3-A.
///
/// No invoca comandos de escritura: consulta las referencias ya presentes en
/// disco. Si el repositorio está recién inicializado, devuelve colecciones
/// vacías en vez de generar un error por no existir todavía HEAD.
pub fn inspect_history(root: &Path) -> Result<GitHistoryOverview, String> {
    let root = checked_root(root)?;
    let has_commits = run_git(Some(&root), &["rev-parse", "--verify", "HEAD"]).is_ok();

    let (graph, revisions) = if has_commits {
        // El grafo de texto lo produce Git: esto evita calcular líneas
        // incorrectas en merges o ramas divergentes.
        let graph = run_git(
            Some(&root),
            &["log", "--graph", "--all", "--decorate", "--oneline", "--max-count=80"],
        )?;
        let log = run_git(
            Some(&root),
            &[
                "log",
                "--all",
                "--topo-order",
                "--max-count=80",
                "--format=%H%x00%h%x00%s",
            ],
        )?;
        (trim_result(&graph), parse_revisions(&log)?)
    } else {
        (String::new(), Vec::new())
    };

    // Git reflog y stash list son consultas: no aplican ni borran entradas.
    let reflog = if has_commits {
        parse_lines(&run_git(
            Some(&root),
            &["reflog", "show", "--max-count=40", "--format=%h %gs"],
        )?)
    } else {
        Vec::new()
    };
    let tags = parse_lines(&run_git(Some(&root), &["tag", "--list", "--sort=refname"])?);
    let stash = parse_lines(&run_git(
        Some(&root),
        &["stash", "list", "--format=%gd %gs"],
    )?);

    // El selector de blame muestra únicamente archivos que ya controla Git.
    let mut tracked_files = parse_paths(&run_git(Some(&root), &["ls-files", "-z"])?);
    let tracked_truncated = tracked_files.len() > MAX_TRACKED_FILES;
    if tracked_truncated {
        tracked_files.truncate(MAX_TRACKED_FILES);
    }

    Ok(GitHistoryOverview {
        root,
        graph,
        revisions,
        reflog,
        tags,
        stash,
        tracked_files,
        tracked_truncated,
    })
}

/// Impide que se usen rutas absolutas, directorios padre o componentes ".git".
///
/// Las rutas vienen del selector GitCraft, pero también se validan en el motor,
/// porque la protección no debe depender únicamente de la interfaz.
fn validate_relative_path(file: &str) -> Result<(), String> {
    let path = Path::new(file);
    if file.is_empty()
        || file.contains('\0')
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("La ruta para blame no es un archivo relativo válido.".into());
    }
    Ok(())
}

/// Rechaza SHA arbitrarios, opciones y expresiones Git que no sean commits.
///
/// Se verifican caracteres hexadecimales y longitud, y posteriormente se
/// solicita a Git que confirme que el objeto es realmente un commit.
fn validate_commit(root: &Path, sha: &str) -> Result<(), String> {
    if !(sha.len() == 40 || sha.len() == 64) || !sha.bytes().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("La revisión debe ser un SHA completo válido.".into());
    }
    let kind = run_git(Some(root), &["cat-file", "-t", sha])?;
    if String::from_utf8_lossy(&kind).trim() != "commit" {
        return Err("El objeto seleccionado no es un commit Git.".into());
    }
    Ok(())
}

/// Compara dos commits seleccionados sin cambiar de rama ni modificar archivos.
///
/// Izquierda y derecha deben ser SHA válidos obtenidos de la lista E3; las
/// diferencias muestran cómo pasar del contenido del primero al segundo.
pub fn compare_commits(root: &Path, left: &str, right: &str) -> Result<String, String> {
    let root = checked_root(root)?;
    validate_commit(&root, left)?;
    validate_commit(&root, right)?;
    if left == right {
        return Err("Selecciona dos commits diferentes para comparar.".into());
    }

    let raw = run_git(
        Some(&root),
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            left,
            right,
            "--",
        ],
    )?;
    Ok(trim_result(&raw))
}

/// Obtiene el origen línea a línea de un archivo ya seguido por Git.
///
/// Se consulta HEAD: las ediciones sin commit no se mezclan con la autoría de
/// versiones confirmadas. Un archivo no seguido no permite usar blame.
pub fn blame_file(root: &Path, file: &str) -> Result<String, String> {
    validate_relative_path(file)?;
    let root = checked_root(root)?;

    // Comprobamos igualdad exacta con rutas de Git para evitar globbing.
    let tracked = parse_paths(&run_git(Some(&root), &["ls-files", "-z", "--", file])?);
    if !tracked.iter().any(|entry| entry == file) {
        return Err("El archivo seleccionado no está seguido por Git.".into());
    }
    let raw = run_git(
        Some(&root),
        &["blame", "--no-textconv", "--date=short", "HEAD", "--", file],
    )?;
    Ok(trim_result(&raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Diferencia cada directorio temporal aunque los tests corran en paralelo.
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    /// Repositorio construido y eliminado exclusivamente por esta prueba.
    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        /// Crea el repositorio con identidad de autor falsa sólo para los tests.
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "gitcraft-e3-{}-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("reloj disponible")
                    .as_nanos()
            ));
            std::fs::create_dir(&root).expect("carpeta E3 de pruebas");
            let fixture = Self { root };
            fixture.git(&["init", "-q"]);
            fixture.git(&["symbolic-ref", "HEAD", "refs/heads/prueba-e3"]);
            fixture.git(&["config", "user.name", "GitCraft E3 Tests"]);
            fixture.git(&["config", "user.email", "gitcraft-e3@example.invalid"]);
            fixture
        }

        /// Ejecuta comandos sólo dentro de la carpeta temporal y sin hooks.
        fn git(&self, args: &[&str]) -> Vec<u8> {
            let mut command = Command::new("git");
            command
                .arg("-C")
                .arg(&self.root)
                .arg("-c")
                .arg("core.hooksPath=/dev/null")
                .arg("-c")
                .arg("commit.gpgsign=false")
                .args(args)
                .env_remove("GIT_DIR")
                .env_remove("GIT_WORK_TREE")
                .env_remove("GIT_INDEX_FILE")
                .env("GIT_TERMINAL_PROMPT", "0");
            let output = command.output().expect("Git instalado para tests");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        }

        /// Crea un commit de prueba sin enviar nada a un servidor.
        fn commit_file(&self, file: &str, text: &str, message: &str) {
            std::fs::write(self.root.join(file), text).expect("escritura de fixture");
            self.git(&["add", "--", file]);
            self.git(&["commit", "-q", "-m", message]);
        }
    }

    impl Drop for Fixture {
        /// Destruye únicamente la carpeta temporal creada por esta prueba.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// El grafo, etiquetas y reflog son legibles y no modifican HEAD.
    #[test]
    fn overview_lists_commits_graph_tags_and_reflog() {
        let fixture = Fixture::new();
        fixture.commit_file("archivo.txt", "primera\n", "Primero");
        fixture.git(&["tag", "test-e3"]);
        fixture.commit_file("archivo.txt", "segunda\n", "Segundo");
        let before_head = fixture.git(&["rev-parse", "HEAD"]);

        let overview = inspect_history(&fixture.root).unwrap();
        assert!(overview.graph.contains("Segundo"));
        assert!(overview.graph.contains("*"));
        assert_eq!(overview.revisions.len(), 2);
        assert!(overview.tags.iter().any(|tag| tag == "test-e3"));
        assert!(!overview.reflog.is_empty());
        assert_eq!(fixture.git(&["rev-parse", "HEAD"]), before_head);
    }

    /// La comparación siempre se realiza entre dos SHA y respeta su orden.
    #[test]
    fn compare_two_commits_is_read_only() {
        let fixture = Fixture::new();
        fixture.commit_file("archivo.txt", "primera\n", "Primero");
        fixture.commit_file("archivo.txt", "segunda\n", "Segundo");
        let history = inspect_history(&fixture.root).unwrap();
        let newer = &history.revisions[0].sha;
        let older = &history.revisions[1].sha;
        let original_head = fixture.git(&["rev-parse", "HEAD"]);
        let diff = compare_commits(&fixture.root, older, newer).unwrap();
        assert!(diff.contains("+segunda"));
        assert!(diff.contains("-primera"));
        assert_eq!(fixture.git(&["rev-parse", "HEAD"]), original_head);
    }

    /// No se aceptan revisiones inyectadas ni objetos que no son commits.
    #[test]
    fn reject_invalid_comparison() {
        let fixture = Fixture::new();
        fixture.commit_file("archivo.txt", "primera\n", "Primero");
        let sha = String::from_utf8(fixture.git(&["rev-parse", "HEAD"]))
            .unwrap()
            .trim()
            .to_string();
        assert!(compare_commits(&fixture.root, "--force", &sha).is_err());
        assert!(compare_commits(&fixture.root, &sha, &sha).is_err());
    }

    /// Blame reconoce rutas con espacios y letras Unicode sin un shell.
    #[test]
    fn blame_accepts_tracked_unicode_paths() {
        let fixture = Fixture::new();
        let name = "archivo de prueba ñ.txt";
        fixture.commit_file(name, "línea de prueba\n", "Autor ficticio");
        let result = blame_file(&fixture.root, name).unwrap();
        assert!(result.contains("línea de prueba"));
        assert!(result.contains("GitCraft E3 Tests"));
    }

    /// No se consulta blame sobre rutas externas ni archivos no seguidos.
    #[test]
    fn blame_rejects_untracked_and_parent_paths() {
        let fixture = Fixture::new();
        fixture.commit_file("archivo.txt", "seguido\n", "Inicial");
        std::fs::write(fixture.root.join("nuevo.txt"), "no seguido\n").unwrap();
        assert!(blame_file(&fixture.root, "../fuera.txt").is_err());
        assert!(blame_file(&fixture.root, "nuevo.txt").is_err());
    }

    /// Un repositorio recién inicializado muestra un historial vacío.
    #[test]
    fn empty_repository_returns_empty_history() {
        let fixture = Fixture::new();
        let history = inspect_history(&fixture.root).unwrap();
        assert!(history.revisions.is_empty());
        assert!(history.graph.is_empty());
        assert!(history.reflog.is_empty());
        assert!(history.tracked_files.is_empty());
    }

    /// El listado de stash es sólo lectura y no aplica cambios guardados.
    #[test]
    fn stash_entries_are_listed_without_changes() {
        let fixture = Fixture::new();
        fixture.commit_file("archivo.txt", "original\n", "Inicial");
        std::fs::write(fixture.root.join("archivo.txt"), "en stash\n").unwrap();
        fixture.git(&["stash", "push", "-m", "Reserva E3"]);
        let before = fixture.git(&["status", "--porcelain"]);
        let history = inspect_history(&fixture.root).unwrap();
        assert!(history.stash.iter().any(|entry| entry.contains("Reserva E3")));
        assert_eq!(fixture.git(&["status", "--porcelain"]), before);
    }
}
