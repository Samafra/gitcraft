//! Motor Git local de E1: inspección sin operaciones de escritura.
//!
//! Este módulo funciona únicamente en aplicaciones de escritorio. Ejecuta el Git
//! instalado por el usuario y utiliza argumentos separados, nunca un intérprete
//! de comandos. Así, una ruta con espacios o caracteres especiales no se ejecuta
//! accidentalmente como una instrucción del sistema.
//!
//! E1 permite ver el estado, las ramas, el historial y los diffs. El módulo
//! hermano local_git_actions implementa las escrituras E2 con confirmación,
//! reutilizando el ejecutor Git común. Ningún subcomando hace push o reset.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Número máximo de caracteres mostrados en un diff para no saturar la interfaz.
const MAX_DIFF_CHARS: usize = 160_000;

/// Estado de un archivo obtenido de Git en formato porcelain v1.
///
/// Los dos caracteres de "code" representan el índice (staging) y el árbol de
/// trabajo, respectivamente. "??" identifica archivos que Git todavía no sigue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFileStatus {
    /// Código Git de dos caracteres, por ejemplo " M", "M " o "??".
    pub code: String,
    /// Ruta relativa al repositorio, que puede contener espacios o Unicode.
    pub path: String,
    /// Ruta anterior si el archivo fue renombrado o copiado.
    pub original_path: Option<String>,
}

/// Referencia local o remota: una rama visible, nunca una orden de checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalBranch {
    /// Nombre completo sin el prefijo refs/heads o refs/remotes.
    pub name: String,
    /// Verdadero si procede de refs/remotes.
    pub remote: bool,
    /// Identificador del commit al que apunta la rama.
    pub commit: String,
}

/// Datos básicos de un commit para mostrar el historial sin modificarlo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCommit {
    /// SHA completo; sólo se usará como argumento después de validarlo.
    pub sha: String,
    /// Versión abreviada del SHA que cabe en una tabla.
    pub short_sha: String,
    /// Autor tal como lo registra el commit.
    pub author: String,
    /// Fecha proporcionada por Git en formato ISO 8601.
    pub date: String,
    /// Primera línea del mensaje del commit.
    pub subject: String,
}

/// Fotografía de un repositorio en el instante en que se ha consultado.
/// Es información en memoria; no se persiste en preferencias ni archivos.
#[derive(Debug, Clone)]
pub struct LocalRepository {
    /// Ruta absoluta del directorio raíz del trabajo Git.
    pub root: PathBuf,
    /// Versión del binario Git usado.
    pub git_version: String,
    /// Rama actual o indicación de HEAD separado.
    pub head: String,
    /// Archivos cambiados y no seguidos, tanto preparados como sin preparar.
    pub status: Vec<LocalFileStatus>,
    /// Ramas locales y referencias remotas disponibles.
    pub branches: Vec<LocalBranch>,
    /// Hasta 50 commits recientes.
    pub commits: Vec<LocalCommit>,
}

/// Define qué diferencia se quiere consultar sin modificar el repositorio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalDiffTarget {
    /// Cambios del directorio de trabajo respecto al índice.
    WorkingTree,
    /// Cambios preparados en el índice respecto a HEAD.
    Staged,
    /// Cambios introducidos por un commit concreto de los del historial.
    Commit(String),
    /// Diferencias de un archivo específico, separando staged de unstaged.
    File { path: String, staged: bool },
}

/// Devuelve la versión del Git instalado o un error claro si no se encuentra.
///
/// Result es un tipo de Rust que devuelve Ok(valor) cuando todo funciona
/// y Err(mensaje) cuando hay un problema. No se requiere autenticación GitHub.
pub fn detect_git() -> Result<String, String> {
    let output = run_git(None, &["--version"])?;
    Ok(String::from_utf8_lossy(&output).trim().to_string())
}

/// Ejecuta un subcomando Git con argumentos independientes y devuelve sus bytes.
///
/// Este ejecutor es compartido por las lecturas E1 y las acciones E2.
/// **No decide si una operación puede escribir:** E2 realiza la comprobación
/// y la confirmación explícita antes de invocar un comando que modifica Git.
///
/// No se utiliza "sh -c", "cmd /C" ni expansión de shell. Se desactiva
/// fsmonitor y los bloqueos opcionales; Git sigue usando los bloqueos
/// obligatorios que necesita para actualizar su índice. Los argumentos
/// sólo los construyen módulos internos, nunca un comando arbitrario de UI.
pub(crate) fn run_git(repo: Option<&Path>, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command
        .arg("--no-optional-locks")
        .arg("-c")
        .arg("core.fsmonitor=false");

    // -C acepta la ruta como un solo argumento, aunque contenga espacios.
    if let Some(directory) = repo {
        command.arg("-C").arg(directory);
    }

    // Cada subcomando y su argumento se añaden de manera literal, sin shell.
    // Este paso fue protegido por un test de regresión en E1 y es reutilizado
    // por las operaciones E2, que sólo se ejecutan tras confirmación.
    command.args(args);

    // La política de entorno está separada para que pueda verificarse.
    configure_git_environment(&mut command);
    let output = command
        .output()
        .map_err(|error| format!("No se pudo ejecutar Git: {}", error))?;

    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let detail = message.trim();
        return Err(if detail.is_empty() {
            format!("Git devolvió el estado {}.", output.status)
        } else {
            format!("Git: {}", detail)
        });
    }
    Ok(output.stdout)
}

/// Aísla el subproceso de variables Git heredadas de otras carpetas.
///
/// No cambia el entorno global de la aplicación ni las variables de Terminal.
/// Sólo configura el programa hijo antes de arrancarlo.
fn configure_git_environment(command: &mut Command) {
    // El index no debe actualizarse por optimizaciones de consulta.
    command
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        // Git puede descargar objetos durante una lectura en clones parciales.
        .env("GIT_NO_LAZY_FETCH", "1")
        // No leer el repositorio de otra sesión de Terminal accidentalmente.
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_COMMON_DIR")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        // El lector de Git local no necesita credenciales GitHub: no pasarlas
        // a procesos externos, aunque GitCraft las haya heredado de Terminal.
        .env_remove("GITHUB_TOKEN")
        .env_remove("GITHUB_PERSONAL_ACCESS_TOKEN")
        .env_remove("GH_TOKEN");
}

/// Interpreta el prefijo ~/ de macOS/Linux sin recurrir a un shell.
///
/// No permite ejecutar comandos ni expandir variables arbitrarias. En Windows
/// también se admite la barra invertida después de ~ cuando USERPROFILE existe.
fn expand_home(input: &str) -> PathBuf {
    let value = input.trim();
    if value == "~" || value.starts_with("~/") || value.starts_with("~\\") {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
        if let Some(home) = home {
            let mut path = PathBuf::from(home);
            let remainder = value
                .trim_start_matches('~')
                .trim_start_matches(|ch| ch == '/' || ch == '\\');
            path.push(remainder);
            return path;
        }
    }
    PathBuf::from(value)
}

/// Abre una carpeta Git local y construye su información de sólo lectura.
///
/// La ruta se normaliza para rechazar errores de carpeta y para que el motor
/// trabaje desde la raíz, incluso si se abre una subcarpeta del repositorio.
/// Los subprocesos de Git sólo leen; ninguna operación envía datos a GitHub.
pub fn inspect_repository(input: &str) -> Result<LocalRepository, String> {
    if input.trim().is_empty() {
        return Err("Escribe la ruta de una carpeta Git local.".into());
    }
    let directory = expand_home(input)
        .canonicalize()
        .map_err(|error| format!("No se puede abrir esa carpeta: {}", error))?;
    if !directory.is_dir() {
        return Err("La ruta indicada no es una carpeta.".into());
    }

    // Se rechazan repositorios bare: E1 necesita un directorio de trabajo.
    let inside = run_git(Some(&directory), &["rev-parse", "--is-inside-work-tree"])?;
    if String::from_utf8_lossy(&inside).trim() != "true" {
        return Err("E1 necesita un repositorio Git con directorio de trabajo.".into());
    }
    let root_bytes = run_git(Some(&directory), &["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(String::from_utf8_lossy(&root_bytes).trim());
    if !root.is_dir() {
        return Err("Git no ha devuelto una carpeta raíz válida.".into());
    }

    let head_bytes = run_git(Some(&root), &["branch", "--show-current"])?;
    let current = String::from_utf8_lossy(&head_bytes);
    let head = if current.trim().is_empty() {
        "(HEAD separado)".to_string()
    } else {
        current.trim().to_string()
    };

    // Los nombres de archivo vienen delimitados por NUL, para soportar
    // espacios, comillas y nombres con saltos de línea sin romper el parseo.
    let status = parse_status(&run_git(
        Some(&root),
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?)?;

    // Git no consulta la red al mostrar refs/remotes: son referencias locales.
    let branches = parse_branches(&run_git(
        Some(&root),
        &[
            "for-each-ref",
            "--format=%(refname)%00%(objectname)",
            "refs/heads",
            "refs/remotes",
        ],
    )?);

    // Un repositorio recién inicializado puede no contener todavía commits.
    // En ese caso mostramos un historial vacío en lugar de un error falso.
    let commits = if run_git(Some(&root), &["rev-parse", "--verify", "HEAD"]).is_ok() {
        parse_commits(&run_git(
            Some(&root),
            &[
                "log",
                "--max-count=50",
                "--date=iso-strict",
                "--format=%H%x00%h%x00%an%x00%ad%x00%s%x00",
            ],
        )?)?
    } else {
        Vec::new()
    };

    Ok(LocalRepository {
        root,
        git_version: detect_git()?,
        head,
        status,
        branches,
        commits,
    })
}

/// Convierte los bytes del estado porcelain -z en archivos individuales.
///
/// Con -z, Git pone primero la ruta nueva y luego la anterior al renombrar:
/// ambas se leen como elementos consecutivos separados por byte cero.
fn parse_status(bytes: &[u8]) -> Result<Vec<LocalFileStatus>, String> {
    let mut pieces = bytes.split(|byte| *byte == 0).filter(|p| !p.is_empty());
    let mut result = Vec::new();

    while let Some(entry) = pieces.next() {
        if entry.len() < 4 || entry[2] != b' ' {
            return Err("La salida de git status no tiene el formato esperado.".into());
        }
        let code = String::from_utf8_lossy(&entry[..2]).into_owned();
        let path = String::from_utf8_lossy(&entry[3..]).into_owned();

        // Renombrar y copiar incluye una segunda ruta en el protocolo -z.
        let original_path = if entry[..2].iter().any(|value| matches!(*value, b'R' | b'C')) {
            let previous = pieces
                .next()
                .ok_or("Falta la ruta original de un archivo renombrado.")?;
            Some(String::from_utf8_lossy(previous).into_owned())
        } else {
            None
        };

        result.push(LocalFileStatus {
            code,
            path,
            original_path,
        });
    }
    Ok(result)
}

/// Lee ramas locales y remotas desde refname; no consulta servidores remotos.
fn parse_branches(bytes: &[u8]) -> Vec<LocalBranch> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter_map(|line| {
            let (reference, commit) = line.split_once('\0')?;
            let (name, remote) = if let Some(name) = reference.strip_prefix("refs/heads/") {
                (name, false)
            } else if let Some(name) = reference.strip_prefix("refs/remotes/") {
                (name, true)
            } else {
                return None;
            };

            // origin/HEAD es un alias simbólico, no una rama independiente.
            if remote && name.ends_with("/HEAD") {
                return None;
            }
            Some(LocalBranch {
                name: name.to_string(),
                remote,
                commit: commit.to_string(),
            })
        })
        .collect()
}

/// Convierte las líneas del historial con campos separados por byte NUL.
///
/// Los cinco campos se generan con --format, sin confiar en el idioma ni en
/// los separadores elegidos por el usuario en su configuración personal.
fn parse_commits(bytes: &[u8]) -> Result<Vec<LocalCommit>, String> {
    let mut commits = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let fields: Vec<&[u8]> = line.split(|byte| *byte == 0).collect();
        if fields.len() < 5 {
            return Err("La salida del historial Git es incompleta.".into());
        }
        let value = |index: usize| String::from_utf8_lossy(fields[index]).into_owned();
        commits.push(LocalCommit {
            sha: value(0),
            short_sha: value(1),
            author: value(2),
            date: value(3),
            subject: value(4),
        });
    }
    Ok(commits)
}

/// Obtiene un diff de trabajo, de staging o de un commit de historial.
///
/// Los commits se validan como SHA hexadecimales antes de dárselos a Git;
/// nunca se interpreta la entrada como opciones ni como un comando externo.
/// La salida larga se recorta para no congelar la interfaz gráfica.
pub fn read_diff(root: &Path, target: &LocalDiffTarget) -> Result<String, String> {
    let bytes = match target {
        LocalDiffTarget::WorkingTree => run_git(
            Some(root),
            &["diff", "--no-ext-diff", "--no-textconv", "--no-color", "--"],
        )?,
        LocalDiffTarget::Staged => run_git(
            Some(root),
            &[
                "diff",
                "--cached",
                "--no-ext-diff",
                "--no-textconv",
                "--no-color",
                "--",
            ],
        )?,
        LocalDiffTarget::File { path, staged } => {
            // La UI obtiene la ruta de git status, no de comandos escritos.
            // Validamos que permanezca dentro de la raíz y usamos "--".
            let candidate = Path::new(path);
            if path.is_empty()
                || candidate.is_absolute()
                || candidate
                    .components()
                    .any(|component| !matches!(component, std::path::Component::Normal(_)))
            {
                return Err("Ruta de archivo inválida para consultar su diff.".into());
            }
            if *staged {
                run_git(
                    Some(root),
                    &[
                        "diff",
                        "--cached",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-color",
                        "--",
                        path,
                    ],
                )?
            } else {
                run_git(
                    Some(root),
                    &[
                        "diff",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--no-color",
                        "--",
                        path,
                    ],
                )?
            }
        }
        LocalDiffTarget::Commit(sha) => {
            if !(sha.len() == 40 || sha.len() == 64)
                || !sha.bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err("El identificador del commit no es un SHA válido.".into());
            }
            run_git(
                Some(root),
                &[
                    "show",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-color",
                    "--format=medium",
                    sha.as_str(),
                    "--",
                ],
            )?
        }
    };
    let content = String::from_utf8_lossy(&bytes);
    let mut output: String = content.chars().take(MAX_DIFF_CHARS).collect();
    if content.chars().count() > MAX_DIFF_CHARS {
        output.push_str("\n\n[Diff truncado para proteger la interfaz]");
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Contador que evita que dos pruebas creen la misma carpeta temporal.
    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    /// Repositorio de prueba creado en el directorio temporal del sistema.
    /// No utiliza ni modifica repositorios existentes del usuario.
    struct TemporaryRepository {
        root: PathBuf,
    }

    impl TemporaryRepository {
        /// Crea una carpeta temporal Git sin commits: se prueba también la
        /// posibilidad de abrir repositorios recién inicializados.
        fn empty() -> Self {
            let id = NEXT_TEST.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "gitcraft-e1-{}-{}-{}",
                std::process::id(),
                id,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("reloj disponible")
                    .as_nanos()
            ));
            std::fs::create_dir(&root).expect("carpeta temporal creada");
            let fixture = Self { root };
            fixture.git(&["init", "-q"]);
            fixture.git(&["symbolic-ref", "HEAD", "refs/heads/main"]);
            fixture
        }

        /// Añade un commit al repositorio temporal, exclusivamente para
        /// que los tests puedan comprobar el historial y las diferencias.
        fn new() -> Self {
            let fixture = Self::empty();
            std::fs::write(fixture.root.join("archivo.txt"), "primera línea\n")
                .expect("archivo de prueba");
            fixture.git(&["add", "archivo.txt"]);
            fixture.git(&[
                "-c",
                "user.name=GitCraft test",
                "-c",
                "user.email=gitcraft@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-q",
                "-m",
                "Primer commit",
            ]);
            fixture
        }

        /// Ejecuta operaciones de preparación únicamente dentro del fixture.
        /// El motor real de E1 nunca utiliza esta función ni escribe archivos.
        fn git(&self, args: &[&str]) -> Vec<u8> {
            let mut command = Command::new("git");
            command
                .arg("-c")
                .arg("core.fsmonitor=false")
                // No ejecutar hooks globales del usuario al crear commits ficticios.
                .arg("-c")
                .arg("core.hooksPath=.git/gitcraft-e1-hooks-disabled")
                .arg("-C")
                .arg(&self.root)
                .args(args);
            // Incluso en los fixtures se evita heredar GIT_DIR del usuario.
            configure_git_environment(&mut command);
            let result = command.output().expect("Git instalado para las pruebas");
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            result.stdout
        }
    }

    impl Drop for TemporaryRepository {
        /// Borra sólo la carpeta única creada por esta prueba al terminar.
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// Garantiza que run_git realmente envía los argumentos a Git.
    /// Previene la regresión que impedía ejecutar status, log y diff en E1.
    #[test]
    fn git_command_forwards_its_arguments() {
        let version = run_git(None, &["--version"]).expect("Git debe recibir --version");
        assert!(
            String::from_utf8_lossy(&version).starts_with("git version"),
            "Git debe ejecutar el argumento recibido, no mostrar su ayuda"
        );
    }

    /// Leer el repositorio no debe cambiar HEAD, archivos ni el índice Git.
    #[test]
    fn inspect_does_not_modify_repository() {
        let fixture = TemporaryRepository::new();
        let before_head = fixture.git(&["rev-parse", "HEAD"]);
        let before_status = fixture.git(&["status", "--porcelain=v1", "-z"]);
        let before_index =
            std::fs::read(fixture.root.join(".git/index")).expect("índice Git de la prueba");
        let snapshot = inspect_repository(fixture.root.to_str().expect("ruta UTF-8"))
            .expect("inspección Git correcta");

        assert_eq!(snapshot.head, "main");
        assert_eq!(snapshot.root, fixture.root.canonicalize().unwrap());
        assert_eq!(snapshot.commits.len(), 1);
        assert!(snapshot.git_version.starts_with("git version"));
        assert_eq!(fixture.git(&["rev-parse", "HEAD"]), before_head);
        assert_eq!(
            fixture.git(&["status", "--porcelain=v1", "-z"]),
            before_status
        );
        assert_eq!(
            std::fs::read(fixture.root.join(".git/index")).unwrap(),
            before_index
        );
    }

    /// Se muestran a la vez cambios preparados, cambios sin preparar y no seguidos.
    #[test]
    fn status_recognizes_staged_unstaged_and_untracked() {
        let fixture = TemporaryRepository::new();
        std::fs::write(fixture.root.join("archivo.txt"), "preparado\n").unwrap();
        fixture.git(&["add", "archivo.txt"]);
        std::fs::write(fixture.root.join("archivo.txt"), "sin preparar\n").unwrap();
        std::fs::write(fixture.root.join("nombre con espacios ñ.txt"), "nuevo\n").unwrap();

        let snapshot = inspect_repository(fixture.root.to_str().unwrap()).unwrap();
        assert!(snapshot
            .status
            .iter()
            .any(|item| { item.code == "MM" && item.path == "archivo.txt" }));
        assert!(snapshot
            .status
            .iter()
            .any(|item| { item.code == "??" && item.path == "nombre con espacios ñ.txt" }));
    }

    /// Una carpeta situada dentro del repositorio se resuelve a la misma raíz.
    #[test]
    fn subdirectory_resolves_to_worktree_root() {
        let fixture = TemporaryRepository::new();
        let nested = fixture.root.join("carpeta con espacios");
        std::fs::create_dir(&nested).unwrap();
        let snapshot = inspect_repository(nested.to_str().unwrap()).unwrap();
        assert_eq!(snapshot.root, fixture.root.canonicalize().unwrap());
    }

    /// Un repositorio Git sin commits también puede abrirse, sin forzar
    /// la creación de una rama ni de un commit desde la aplicación.
    #[test]
    fn empty_repository_is_supported() {
        let fixture = TemporaryRepository::empty();
        let snapshot = inspect_repository(fixture.root.to_str().unwrap()).unwrap();
        assert!(snapshot.commits.is_empty());
        assert!(snapshot.status.is_empty());
        assert_eq!(snapshot.head, "main");
    }

    /// El protocolo -z conserva los nombres original y nuevo de un renombrado.
    #[test]
    fn renames_are_parsed_without_losing_paths() {
        let bytes = b"R  nombre nuevo.txt\0nombre antiguo.txt\0";
        let entries = parse_status(bytes).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "nombre nuevo.txt");
        assert_eq!(
            entries[0].original_path.as_deref(),
            Some("nombre antiguo.txt")
        );
    }

    /// Las ramas creadas en un repositorio temporal son sólo datos de lectura.
    #[test]
    fn branches_and_history_are_available() {
        let fixture = TemporaryRepository::new();
        fixture.git(&["branch", "experimento"]);
        let snapshot = inspect_repository(fixture.root.to_str().unwrap()).unwrap();
        assert!(snapshot
            .branches
            .iter()
            .any(|branch| { branch.name == "experimento" && !branch.remote }));
        assert_eq!(snapshot.commits[0].subject, "Primer commit");
        assert_eq!(snapshot.commits[0].sha.len(), 40);
    }

    /// Consultar diferencias no altera el contenido de archivos ni el commit.
    #[test]
    fn diffs_are_read_only_and_reject_bad_revisions() {
        let fixture = TemporaryRepository::new();
        std::fs::write(fixture.root.join("archivo.txt"), "cambio sin preparar\n").unwrap();
        let sha_before = fixture.git(&["rev-parse", "HEAD"]);
        let diff = read_diff(&fixture.root, &LocalDiffTarget::WorkingTree).unwrap();
        assert!(diff.contains("cambio sin preparar"));
        let diff_staged = read_diff(&fixture.root, &LocalDiffTarget::Staged).unwrap();
        assert!(diff_staged.is_empty());
        assert!(read_diff(
            &fixture.root,
            &LocalDiffTarget::Commit("--evil".to_string())
        )
        .is_err());
        assert_eq!(fixture.git(&["rev-parse", "HEAD"]), sha_before);
    }

    /// El motor no debe depender de las variables de otro repositorio
    /// heredadas del proceso padre, aunque se haya lanzado desde esa carpeta.
    #[test]
    fn git_directory_environment_does_not_redirect_a_read() {
        let fixture = TemporaryRepository::new();
        // Este test valida la política del Command sin alterar el entorno
        // global del proceso, para no interferir con tests paralelos.
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(&fixture.root)
            .arg("rev-parse")
            .arg("--show-toplevel")
            .env("GIT_DIR", "/no/existe/.git");
        // Invocamos la misma política que utiliza run_git en producción.
        configure_git_environment(&mut command);
        let output = command.output().expect("Git disponible");
        assert!(output.status.success());
        // macOS puede representar /var como /private/var: comparar rutas
        // normalizadas evita confundir un enlace simbólico con otro repositorio.
        let reported = String::from_utf8_lossy(&output.stdout);
        let root = PathBuf::from(reported.trim());
        assert_eq!(
            root.canonicalize().unwrap(),
            fixture.root.canonicalize().unwrap()
        );
    }

    /// Una carpeta que no es Git debe devolver error y no inicializarse.
    #[test]
    fn ordinary_directory_is_rejected_without_changes() {
        // Usamos una carpeta distinta del fixture porque una subcarpeta Git
        // también debe ser reconocida correctamente por inspect_repository.
        let isolated = std::env::temp_dir().join(format!(
            "gitcraft-no-repo-{}-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("reloj disponible")
                .as_nanos()
        ));
        std::fs::create_dir(&isolated).unwrap();
        let result = inspect_repository(isolated.to_str().unwrap());
        assert!(result.is_err());
        assert!(!isolated.join(".git").exists());
        std::fs::remove_dir(&isolated).unwrap();
    }
}
