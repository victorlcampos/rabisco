//! Atualização pelo próprio app: baixa o Rabisco.zip da última release do GitHub, confere o
//! SHA-256 e a versão (cerne), troca o Rabisco.app instalado de uma vez só e reabre o Rabisco
//! na barra de menus já na versão nova. Pelo menu (*Procurar atualização*) ou pelo terminal:
//! `/Applications/Rabisco.app/Contents/MacOS/rabisco update [--check]`.

use std::fs;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use cerne::update::{Error, Updater, Version};

use crate::login;

const REPO: &str = "victorlcampos/rabisco";
/// O app zipado que a release publica (scripts/bundle.sh e o workflow de release).
const ASSET: &str = "Rabisco.zip";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub enum Outcome {
    /// Esta já é a última versão (ou uma mais nova que ela).
    UpToDate,
    /// Há uma versão nova, que não foi instalada (`--check`).
    Available(Version),
    /// A versão nova está no lugar da antiga; este processo segue na antiga até reabrir.
    Installed(Version),
}

/// Procura a última versão e, se ela for mais nova, instala no lugar de `bundle` (com
/// `install` desligado, só conta). `found` fica sabendo da versão nova antes do download.
pub fn update(
    bundle: &Path,
    install: bool,
    found: impl FnOnce(&Version),
) -> Result<Outcome, Error> {
    let updater = Updater::new(REPO, "rabisco", VERSION)?;
    let release = updater.latest()?;
    if release.version <= *updater.current() {
        return Ok(Outcome::UpToDate);
    }
    if !install {
        return Ok(Outcome::Available(release.version));
    }
    found(&release.version);
    updater.download_app(&release, ASSET, bundle)?.install()?;
    Ok(Outcome::Installed(release.version))
}

/// O Rabisco.app de onde este processo roda, se for um app instalado: fica de fora o
/// `cargo run` e a cópia temporária que o Gatekeeper usa quando o app é aberto direto da
/// pasta Downloads ("App Translocation"), onde não dá para gravar e um início automático
/// apontando para ela quebraria.
pub fn installed_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    bundle_of(&exe.canonicalize().unwrap_or(exe))
}

/// `…/Rabisco.app` para `…/Rabisco.app/Contents/MacOS/rabisco`.
fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let inside = macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && app.extension()? == "app";
    (inside && !app.to_string_lossy().contains("/AppTranslocation/")).then(|| app.to_owned())
}

/// Reabre o Rabisco quando o processo `pid` terminar: pelo launchd, se o início com o Mac
/// estiver carregado (assim ele continua cuidando do app), senão pelo `open`.
pub fn reopen_after(pid: u32, bundle: &Path) -> std::io::Result<()> {
    const SCRIPT: &str = r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done
launchctl kickstart "gui/$(id -u)/$2" 2>/dev/null || exec open -g -a "$3" --args --background"#;
    Command::new("/bin/sh")
        .args(["-c", SCRIPT, "sh"])
        .arg(pid.to_string())
        .arg(login::LABEL)
        .arg(bundle)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        // Grupo de processos próprio: o launchd encerra o grupo do Rabisco quando ele sai.
        .process_group(0)
        .spawn()
        .map(drop)
}

/// O Rabisco que está aberto na barra de menus, visto de outro processo (o do terminal).
enum Running {
    No,
    /// O PID que ele anotou na trava, conferido pelo caminho do executável.
    Pid(i32),
    /// Aberto, mas sem PID anotado: versões até a 0.1.2.
    Unknown,
}

fn running(lock: &Path) -> Running {
    let Ok(file) = fs::File::open(lock) else {
        return Running::No;
    };
    // Conseguir a trava quer dizer que ninguém a segura; ela se solta ao fechar o arquivo.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Running::No;
    }
    let pid = fs::read_to_string(lock)
        .ok()
        .and_then(|text| text.trim().parse::<i32>().ok());
    match pid {
        Some(pid) if is_rabisco(pid) => Running::Pid(pid),
        _ => Running::Unknown,
    }
}

/// Se o processo `pid` roda o executável de um Rabisco.app: um PID velho nunca encerra outro
/// programa.
fn is_rabisco(pid: i32) -> bool {
    let mut path = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let len = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), path.len() as u32) };
    len > 0 && path[..len as usize].ends_with(b".app/Contents/MacOS/rabisco")
}

/// `rabisco update [--check]` no terminal; devolve o código de saída.
pub fn cli(args: &[String]) -> i32 {
    let check = args.iter().any(|arg| arg == "--check");
    let Some(bundle) = installed_bundle() else {
        eprintln!(
            "O update troca o Rabisco.app instalado: rode o rabisco de dentro dele \
             (/Applications/Rabisco.app/Contents/MacOS/rabisco update)."
        );
        return 1;
    };
    // Visto antes da troca: depois dela, o executável do processo aberto já não existe.
    let running = running(&crate::lock_path());
    eprintln!("Procurando uma versão nova do Rabisco…");
    let result = update(&bundle, !check, |version| {
        eprintln!("Baixando o Rabisco {version}…");
    });
    match result {
        Err(error) => {
            eprintln!("Erro ao atualizar: {error}");
            1
        }
        Ok(Outcome::UpToDate) => {
            println!("O Rabisco {VERSION} é a versão mais nova.");
            0
        }
        Ok(Outcome::Available(version)) => {
            println!(
                "O Rabisco {version} está disponível (este é o {VERSION}): `rabisco update` instala."
            );
            0
        }
        Ok(Outcome::Installed(version)) => {
            println!(
                "Rabisco atualizado: {VERSION} → {version} ({})",
                bundle.display()
            );
            match running {
                Running::Pid(pid) => {
                    let reopened = reopen_after(pid as u32, &bundle);
                    // Sai com sucesso (main.rs), e o launchd não o reabre por conta própria.
                    unsafe { libc::kill(pid, libc::SIGTERM) };
                    match reopened {
                        Ok(()) => println!("Reabrindo o Rabisco na barra de menus."),
                        Err(error) => {
                            println!("Abra o Rabisco de novo para usar a versão nova ({error}).")
                        }
                    }
                }
                Running::Unknown => println!(
                    "O Rabisco aberto na barra de menus segue na versão antiga até ser reaberto: \
                     Sair do Rabisco, no menu, e abrir de novo."
                ),
                Running::No => {}
            }
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Um arquivo de trava numa pasta nova só para o teste.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rabisco-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("rabisco.lock")
    }

    #[test]
    fn finds_the_bundle_around_the_executable() {
        let bundle = |path: &str| bundle_of(Path::new(path));
        assert_eq!(
            bundle("/Applications/Rabisco.app/Contents/MacOS/rabisco"),
            Some(PathBuf::from("/Applications/Rabisco.app"))
        );
        assert_eq!(
            bundle("/Users/eu/Applications/Rabisco.app/Contents/MacOS/rabisco"),
            Some(PathBuf::from("/Users/eu/Applications/Rabisco.app"))
        );
        assert_eq!(bundle("/Users/eu/rabisco/target/release/rabisco"), None);
        assert_eq!(bundle("/Applications/Rabisco/Contents/MacOS/rabisco"), None);
        assert_eq!(
            bundle(
                "/private/var/folders/x/AppTranslocation/1234/d/Rabisco.app/Contents/MacOS/rabisco"
            ),
            None
        );
    }

    #[test]
    fn a_free_lock_means_rabisco_is_not_open() {
        let lock = scratch("free");
        assert!(matches!(running(&lock), Running::No), "sem arquivo");
        fs::write(&lock, std::process::id().to_string()).unwrap();
        assert!(matches!(running(&lock), Running::No), "arquivo sem trava");
        let _ = fs::remove_dir_all(lock.parent().unwrap());
    }

    #[test]
    fn a_held_lock_names_rabisco_only_by_its_executable() {
        let lock = scratch("held");
        // Este processo (o dos testes) segura a trava com o próprio PID anotado.
        fs::write(&lock, std::process::id().to_string()).unwrap();
        let held = fs::File::open(&lock).unwrap();
        assert_eq!(
            unsafe { libc::flock(held.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
            0
        );
        assert!(
            matches!(running(&lock), Running::Unknown),
            "o PID é de outro programa (o executável dos testes)"
        );
        fs::write(&lock, "").unwrap();
        assert!(
            matches!(running(&lock), Running::Unknown),
            "sem PID: versão antiga"
        );
        drop(held);
        assert!(matches!(running(&lock), Running::No));
        let _ = fs::remove_dir_all(lock.parent().unwrap());
    }

    #[test]
    fn other_programs_are_not_rabisco() {
        assert!(!is_rabisco(std::process::id() as i32));
        assert!(!is_rabisco(1), "o launchd");
        assert!(!is_rabisco(-1));
    }
}
