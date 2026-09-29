//! Início junto com o Mac, via LaunchAgent em ~/Library/LaunchAgents.
//!
//! O plist aponta para o executável dentro do .app e passa `--background`, para o
//! app subir só na barra de menus. Com `KeepAlive/SuccessfulExit = false`, o launchd
//! reabre o Rabisco se ele travar, mas respeita o "Sair" do menu.

use std::path::PathBuf;

pub const LABEL: &str = "io.github.victorlcampos.rabisco";

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

pub fn plist_path() -> PathBuf {
    home()
        .join("Library/LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

pub fn log_path() -> PathBuf {
    home().join("Library/Logs/Rabisco.log")
}

pub fn is_enabled() -> bool {
    plist_path().exists()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn plist(exe: &str) -> String {
    let exe = xml_escape(exe);
    let log = xml_escape(&log_path().to_string_lossy());
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{exe}</string>
		<string>--background</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<dict>
		<key>SuccessfulExit</key>
		<false/>
	</dict>
	<key>ProcessType</key>
	<string>Interactive</string>
	<key>LimitLoadToSessionType</key>
	<string>Aqua</string>
	<key>AssociatedBundleIdentifiers</key>
	<array>
		<string>{LABEL}</string>
	</array>
	<key>StandardOutPath</key>
	<string>{log}</string>
	<key>StandardErrorPath</key>
	<string>{log}</string>
</dict>
</plist>
"#
    )
}

fn current_exe() -> std::io::Result<String> {
    Ok(std::env::current_exe()?
        .canonicalize()?
        .to_string_lossy()
        .into_owned())
}

pub fn enable() -> std::io::Result<()> {
    let path = plist_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, plist(&current_exe()?))
}

/// Desliga o início automático. O processo atual continua rodando.
pub fn disable() -> std::io::Result<()> {
    match std::fs::remove_file(plist_path()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Se o app mudou de lugar (ex.: foi reinstalado em outra pasta), atualiza o plist.
pub fn refresh_if_moved() {
    let Ok(exe) = current_exe() else { return };
    if let Ok(content) = std::fs::read_to_string(plist_path())
        && !content.contains(&format!("<string>{}</string>", xml_escape(&exe)))
    {
        let _ = std::fs::write(plist_path(), plist(&exe));
    }
}
