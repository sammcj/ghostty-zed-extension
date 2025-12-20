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

        // Try to download from GitHub releases
        if let Ok(()) = self.try_download_binary(binary_name, os, arch) {
            self.cached_binary_path = Some(binary_name.to_string());
        }

        // Return binary name regardless - if download failed but binary exists locally, it will work
        binary_name.to_string()
    }

    fn try_download_binary(
        &self,
        binary_name: &str,
        os: zed::Os,
        arch: zed::Architecture,
    ) -> std::result::Result<(), String> {
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

        zed::download_file(
            &asset.download_url,
            binary_name,
            zed::DownloadedFileType::GzipTar,
        )
        .map_err(|e| e.to_string())?;

        zed::make_file_executable(binary_name).map_err(|e| e.to_string())?;

        Ok(())
    }
}

zed::register_extension!(GhosttyExtension);
