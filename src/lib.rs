use std::fs;

use zed_extension_api::{self as zed, LanguageServerId, Result};

struct GhosttyExtension {
    cached_binary_path: Option<String>,
}

impl zed::Extension for GhosttyExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        let env = worktree.shell_env();

        // Check for custom path: set GHOSTTY_LSP_PATH=/path/to/ghostty-lsp in your shell
        let binary_path = env
            .iter()
            .find(|(k, _)| k == "GHOSTTY_LSP_PATH")
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| self.get_or_download_lsp_binary());

        Ok(zed::Command {
            command: binary_path,
            args: vec![],
            env,
        })
    }
}

impl GhosttyExtension {
    fn get_or_download_lsp_binary(&mut self) -> String {
        if let Some(path) = &self.cached_binary_path {
            return path.clone();
        }

        let (os, arch) = zed::current_platform();

        let binary_name = match os {
            zed::Os::Mac | zed::Os::Linux => "ghostty-lsp",
            zed::Os::Windows => "ghostty-lsp.exe",
        };

        let stem = binary_name.strip_suffix(".exe").unwrap_or(binary_name);

        // 1) Try resolving an existing install (covers the current broken layout too).
        if let Some(path) = Self::resolve_existing_binary_path(binary_name, stem) {
            let _ = zed::make_file_executable(&path);
            self.cached_binary_path = Some(path.clone());
            return path;
        }

        // 2) Try download, then resolve again.
        if let Ok(path) = self.try_download_binary(binary_name, stem, os, arch) {
            self.cached_binary_path = Some(path.clone());
            return path;
        }

        // Fallback: let Zed try PATH, or let the user set GHOSTTY_LSP_PATH.
        binary_name.to_string()
    }

    fn resolve_existing_binary_path(binary_name: &str, stem: &str) -> Option<String> {
        // Common layouts depending on DownloadedFileType and archive contents:
        // - Gzip:         ./ghostty-lsp
        // - GzipTar:      ./ghostty-lsp/ghostty-lsp   (current reported bug)
        // - Sometimes:    ./ghostty-lsp/ghostty-lsp/ghostty-lsp (tar contains top-level dir)
        let candidates: [String; 4] = [
            binary_name.to_string(),
            format!("{stem}/{binary_name}"),
            format!("{binary_name}/{binary_name}"),
            format!("{binary_name}/{stem}/{binary_name}"),
        ];

        for candidate in candidates {
            if fs::metadata(&candidate).map(|m| m.is_file()).unwrap_or(false) {
                return Some(candidate);
            }
        }

        None
    }

    fn try_download_binary(
        &self,
        binary_name: &str,
        stem: &str,
        os: zed::Os,
        arch: zed::Architecture,
    ) -> std::result::Result<String, String> {
        let os_name = match os {
            zed::Os::Mac => "darwin",
            zed::Os::Linux => "linux",
            zed::Os::Windows => "windows",
        };

        let arch_name = match arch {
            zed::Architecture::Aarch64 => "aarch64",
            zed::Architecture::X8664 => "x86_64",
            _ => return Err("Unsupported architecture".to_string()),
        };

        let asset_name = format!("ghostty-lsp-{}-{}.tar.gz", os_name, arch_name);

        let release = zed::latest_github_release(
            "Else00/ghostty-zed-extension",
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )
        .map_err(|e| e.to_string())?;

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("No asset found for {}", asset_name))?;

        // Use a versioned folder (pattern used by other Zed extensions) so updates work cleanly.
        let version_dir = format!("ghostty-lsp-{}", release.version);

        // If already present, just resolve and ensure it's executable.
        if let Some(path) = Self::resolve_in_version_dir(&version_dir, binary_name, stem) {
            zed::make_file_executable(&path).map_err(|e| e.to_string())?;
            return Ok(path);
        }

        zed::download_file(
            &asset.download_url,
            &version_dir,
            zed::DownloadedFileType::GzipTar,
        )
        .map_err(|e| e.to_string())?;

        let path = Self::resolve_in_version_dir(&version_dir, binary_name, stem).ok_or_else(|| {
            format!(
                "Downloaded {}, but couldn't find extracted binary inside {}",
                asset_name, version_dir
            )
        })?;

        zed::make_file_executable(&path).map_err(|e| e.to_string())?;

        // Cleanup old versions (only folders that look like ours).
        if let Ok(entries) = fs::read_dir(".") {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let Some(name) = file_name.to_str() else {
                    continue;
                };
                if name == version_dir {
                    continue;
                }
                if name.starts_with("ghostty-lsp-") {
                    let _ = fs::remove_dir_all(entry.path());
                }
            }
        }

        Ok(path)
    }

    fn resolve_in_version_dir(version_dir: &str, binary_name: &str, stem: &str) -> Option<String> {
        let candidates: [String; 3] = [
            format!("{version_dir}/{binary_name}"),
            format!("{version_dir}/{stem}/{binary_name}"),
            format!("{version_dir}/{binary_name}/{binary_name}"),
        ];

        for candidate in candidates {
            if fs::metadata(&candidate).map(|m| m.is_file()).unwrap_or(false) {
                return Some(candidate);
            }
        }

        None
    }
}

zed::register_extension!(GhosttyExtension);
