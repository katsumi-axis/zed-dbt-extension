use std::{collections::HashMap, fs};
use zed::settings::LspSettings;
use zed_extension_api as zed;

const LANGUAGE_SERVER_NAME: &str = "dbt-fusion";
const DBT_BINARY: &str = "dbt";
const DBT_LSP_BINARY: &str = "dbt-lsp";
const DBT_LSP_PROXY_BINARY: &str = "dbt-lsp-proxy";
const DBT_LSP_PROXY_REPOSITORY: &str = "katsumi-axis/zed-dbt-fusion";
const DBT_BINARY_PATH_ENV: &str = "DBT_BINARY_PATH";

struct DbtExtension;

struct ProxyPlatform {
    asset_name: &'static str,
    binary_path: String,
    needs_executable_bit: bool,
}

impl DbtExtension {
    fn dbt_project_dir(worktree: &zed::Worktree) -> zed::Result<String> {
        worktree
            .read_text_file("dbt_project.yml")
            .map(|_| worktree.root_path())
            .map_err(|_| {
                "dbt Fusion language server requires dbt_project.yml at the Zed worktree root."
                    .into()
            })
    }

    fn env_for_lsp(
        worktree: &zed::Worktree,
        dbt_project_dir: &str,
        configured_env: Option<HashMap<String, String>>,
        extra_env: Vec<(String, String)>,
    ) -> Vec<(String, String)> {
        let mut env = worktree.shell_env();

        if let Some(configured_env) = configured_env {
            for (key, value) in configured_env {
                upsert_env(&mut env, key, value);
            }
        }

        upsert_env(&mut env, "DBT_PROJECT_DIR".into(), dbt_project_dir.into());
        for (key, value) in extra_env {
            upsert_env(&mut env, key, value);
        }
        env
    }

    fn command(
        worktree: &zed::Worktree,
        dbt_project_dir: &str,
        command: String,
        args: Vec<String>,
        configured_env: Option<HashMap<String, String>>,
        extra_env: Vec<(String, String)>,
    ) -> zed::Command {
        zed::Command {
            command,
            args,
            env: Self::env_for_lsp(worktree, dbt_project_dir, configured_env, extra_env),
        }
    }

    fn resolve_path(worktree: &zed::Worktree, path: &str) -> zed::Result<String> {
        if path.contains('/') {
            return Ok(path.into());
        }

        worktree
            .which(path)
            .ok_or_else(|| format!("{path} was not found on the Zed worktree PATH."))
    }

    fn proxy_release_tag() -> String {
        format!("v{}", env!("CARGO_PKG_VERSION"))
    }

    fn proxy_platform() -> zed::Result<ProxyPlatform> {
        let (os, architecture) = zed::current_platform();
        let asset_name = match (os, architecture) {
            (zed::Os::Mac, zed::Architecture::Aarch64) => "dbt-lsp-proxy-aarch64-apple-darwin.gz",
            (zed::Os::Mac, zed::Architecture::X8664) => "dbt-lsp-proxy-x86_64-apple-darwin.gz",
            (zed::Os::Linux, zed::Architecture::Aarch64) => {
                "dbt-lsp-proxy-aarch64-unknown-linux-gnu.gz"
            }
            (zed::Os::Linux, zed::Architecture::X8664) => {
                "dbt-lsp-proxy-x86_64-unknown-linux-gnu.gz"
            }
            (zed::Os::Windows, zed::Architecture::X8664) => {
                "dbt-lsp-proxy-x86_64-pc-windows-msvc.exe.gz"
            }
            _ => {
                return Err(format!(
                    "dbt-lsp-proxy is not available for {os:?}/{architecture:?}."
                ));
            }
        };

        let extension = if os == zed::Os::Windows { ".exe" } else { "" };
        Ok(ProxyPlatform {
            asset_name,
            binary_path: format!(
                "{DBT_LSP_PROXY_BINARY}-{}{}",
                env!("CARGO_PKG_VERSION"),
                extension
            ),
            needs_executable_bit: os != zed::Os::Windows,
        })
    }

    fn resolve_proxy_path(
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<String> {
        if let Some(command) = worktree.which(DBT_LSP_PROXY_BINARY) {
            return Ok(command);
        }

        Self::download_proxy(language_server_id)
    }

    fn download_proxy(language_server_id: &zed::LanguageServerId) -> zed::Result<String> {
        let result = Self::download_proxy_inner(language_server_id);
        match &result {
            Ok(_) => zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::None,
            ),
            Err(error) => zed::set_language_server_installation_status(
                language_server_id,
                &zed::LanguageServerInstallationStatus::Failed(error.clone()),
            ),
        }
        result
    }

    fn download_proxy_inner(language_server_id: &zed::LanguageServerId) -> zed::Result<String> {
        let platform = Self::proxy_platform()?;
        if fs::metadata(&platform.binary_path).is_ok_and(|metadata| metadata.is_file()) {
            return Ok(platform.binary_path);
        }

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::CheckingForUpdate,
        );

        let release_tag = Self::proxy_release_tag();
        let release = zed::github_release_by_tag_name(DBT_LSP_PROXY_REPOSITORY, &release_tag)
            .map_err(|error| {
                format!("failed to fetch {DBT_LSP_PROXY_REPOSITORY} release {release_tag}: {error}")
            })?;
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == platform.asset_name)
            .ok_or_else(|| {
                format!(
                    "{platform_asset} was not found in {repository} release {release_tag}.",
                    platform_asset = platform.asset_name,
                    repository = DBT_LSP_PROXY_REPOSITORY
                )
            })?;

        zed::set_language_server_installation_status(
            language_server_id,
            &zed::LanguageServerInstallationStatus::Downloading,
        );
        zed::download_file(
            &asset.download_url,
            &platform.binary_path,
            zed::DownloadedFileType::Gzip,
        )
        .map_err(|error| format!("failed to download {}: {error}", asset.name))?;

        if platform.needs_executable_bit {
            zed::make_file_executable(&platform.binary_path).map_err(|error| {
                format!(
                    "failed to make {} executable: {error}",
                    platform.binary_path
                )
            })?;
        }

        Ok(platform.binary_path)
    }

    fn default_command(
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
        dbt_project_dir: &str,
    ) -> zed::Result<zed::Command> {
        if let Some(command) = worktree.which(DBT_LSP_BINARY) {
            return Ok(Self::command(
                worktree,
                dbt_project_dir,
                command,
                Vec::new(),
                None,
                Vec::new(),
            ));
        }

        if let Some(dbt_command) = worktree.which(DBT_BINARY) {
            return Ok(Self::command(
                worktree,
                dbt_project_dir,
                Self::resolve_proxy_path(language_server_id, worktree)?,
                Vec::new(),
                None,
                vec![(DBT_BINARY_PATH_ENV.into(), dbt_command)],
            ));
        }

        Err(format!(
            "{DBT_LSP_BINARY} and {DBT_BINARY} were not found on the Zed worktree PATH."
        ))
    }

    fn command_from_settings(
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
        dbt_project_dir: &str,
    ) -> zed::Result<Option<zed::Command>> {
        let settings = LspSettings::for_worktree(LANGUAGE_SERVER_NAME, worktree)?;
        let Some(binary) = settings.binary else {
            return Ok(None);
        };

        let Some(command) = binary.path else {
            return Ok(None);
        };

        let mut args = binary.arguments.unwrap_or_default();
        if (command == DBT_BINARY || command.ends_with("/dbt"))
            && args.first().is_some_and(|arg| arg == "lsp")
        {
            let dbt_command = Self::resolve_path(worktree, &command)?;
            args.remove(0);
            return Ok(Some(Self::command(
                worktree,
                dbt_project_dir,
                Self::resolve_proxy_path(language_server_id, worktree)?,
                args,
                binary.env,
                vec![(DBT_BINARY_PATH_ENV.into(), dbt_command)],
            )));
        }

        let command = Self::resolve_path(worktree, &command)?;

        Ok(Some(Self::command(
            worktree,
            dbt_project_dir,
            command,
            args,
            binary.env,
            Vec::new(),
        )))
    }
}

fn upsert_env(env: &mut Vec<(String, String)>, key: String, value: String) {
    if let Some((_, existing_value)) = env
        .iter_mut()
        .find(|(existing_key, _)| existing_key == &key)
    {
        *existing_value = value;
    } else {
        env.push((key, value));
    }
}

impl zed::Extension for DbtExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<zed::Command> {
        let dbt_project_dir = Self::dbt_project_dir(worktree)?;

        if let Some(command) =
            Self::command_from_settings(language_server_id, worktree, &dbt_project_dir)?
        {
            return Ok(command);
        }

        Self::default_command(language_server_id, worktree, &dbt_project_dir)
    }

    fn language_server_initialization_options(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(LANGUAGE_SERVER_NAME, worktree)?;
        Ok(settings.initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        _language_server_id: &zed::LanguageServerId,
        worktree: &zed::Worktree,
    ) -> zed::Result<Option<zed::serde_json::Value>> {
        let settings = LspSettings::for_worktree(LANGUAGE_SERVER_NAME, worktree)?;
        Ok(settings.settings)
    }
}

zed::register_extension!(DbtExtension);
