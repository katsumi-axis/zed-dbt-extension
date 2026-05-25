# dbt for Zed

Experimental dbt support for Zed, designed around the dbt Fusion language server.

## What works now

- Registers a `dbt SQL` language for `.sql` files.
- Starts the `dbt-fusion` language server entry for `dbt SQL`.
- Runs `dbt-lsp` from the normal process `PATH` by default, or falls back to a
  stdio-to-socket proxy for unified `dbt lsp` installs.
- Allows overriding the language server binary and arguments through Zed settings.
- Sets `DBT_PROJECT_DIR` for the language server after locating `dbt_project.yml`.
- Adds Zed tasks for running, building, testing, compiling, and previewing the current dbt model file.
- Adds initial dbt snippets.

## Requirements

- Zed
- Rust installed with `rustup` for dev-extension builds
- dbt Fusion available as either `dbt-lsp` or the unified `dbt` binary
- A Zed worktree whose root contains `dbt_project.yml`

## Local development

Install this repository as a dev extension:

1. Build the native proxy used for unified `dbt lsp` installs:

```sh
cargo build --release --bin dbt-lsp-proxy
```

2. For dev-extension testing before release assets exist, install the proxy on
   the `PATH` Zed receives:

```sh
cargo install --path . --bin dbt-lsp-proxy
```

3. Open the Zed command palette.
4. Run `zed: install dev extension`.
5. Select this repository directory.
6. Open `zed: open log` if the extension fails to build or the language server does not start.

## Configuration

By default, the extension looks for `dbt-lsp` on the Zed worktree `PATH` and
starts the resolved executable path. If `dbt-lsp` is not available, it falls back
to `dbt-lsp-proxy`, which starts `dbt lsp --socket` and bridges it to Zed's
stdio-based LSP transport.

For the proxy fallback, the extension first checks for `dbt-lsp-proxy` on the
worktree `PATH`. If it is not present, it fetches the matching platform asset
from this repository's GitHub release whose tag matches the extension version,
such as `v0.0.1`. The extension uses GitHub release metadata instead of
hard-coded asset URLs or local absolute paths.

The `dbt-lsp` default is equivalent to configuring:

```json
{
  "lsp": {
    "dbt-fusion": {
      "binary": {
        "path": "dbt-lsp",
        "arguments": []
      }
    }
  }
}
```

The unified `dbt` fallback is handled automatically by the extension. If you want
to bypass the proxy and test manually, the underlying command is:

```sh
dbt lsp --socket <port> --project-dir <path-to-dbt-project> --quiet
```

It starts only when it can find `dbt_project.yml` at the worktree root.

When the language server starts, the extension passes the current shell
environment and sets `DBT_PROJECT_DIR` to the detected dbt project directory.
For unified `dbt lsp` installs, the proxy writes concise lifecycle logs to Zed's
log with the `dbt-lsp-proxy:` prefix.
The proxy also filters `workspace/didChangeConfiguration`, which Zed sends but
dbt Fusion `2.0.0-preview.177` currently reports as an unsupported notification.

## Proxy release assets

Before publishing a version that supports unified `dbt lsp`, attach gzipped
`dbt-lsp-proxy` binaries to the matching GitHub release tag:

- `dbt-lsp-proxy-aarch64-apple-darwin.gz`
- `dbt-lsp-proxy-x86_64-apple-darwin.gz`
- `dbt-lsp-proxy-aarch64-unknown-linux-gnu.gz`
- `dbt-lsp-proxy-x86_64-unknown-linux-gnu.gz`
- `dbt-lsp-proxy-x86_64-pc-windows-msvc.exe.gz`

## Running the current model

`dbt SQL` files include language-provided Zed tasks:

- `dbt show current model`
- `dbt build current model`
- `dbt run current model`
- `dbt test current model`
- `dbt compile current model`

Use `task: spawn` from a dbt model file to run them. A runnable marker is also
attached to the file and defaults to `dbt show current model`, which previews up
to 100 rows in Zed's integrated terminal by calling `dbt show --select
path:<current-file> --limit 100`.

The tasks save the current buffer before running and use the Zed worktree root
as the dbt project directory.

If your Fusion language server uses a different command, configure it in Zed settings:

```json
{
  "lsp": {
    "dbt-fusion": {
      "binary": {
        "path": "/absolute/path/to/dbt-lsp",
        "arguments": []
      }
    }
  }
}
```

If the Fusion binary exposes the LSP through `dbtf`, configure it explicitly:

```json
{
  "lsp": {
    "dbt-fusion": {
      "binary": {
        "path": "/absolute/path/to/dbtf",
        "arguments": ["lsp"]
      }
    }
  }
}
```

## Publishing to the Zed extension registry

Zed extensions are published through the central `zed-industries/extensions` repository.

The flow is:

1. Push this extension repository to GitHub.
2. Create a GitHub release whose tag matches `version` in `extension.toml`.
3. Attach the proxy release assets listed above.
4. Fork `zed-industries/extensions`.
5. Add this repository as an HTTPS Git submodule under `extensions/dbt`.
6. Add an entry to the top-level `extensions.toml`:

```toml
[dbt]
submodule = "extensions/dbt"
version = "0.0.1"
```

7. Run `pnpm sort-extensions`.
8. Open a PR to `zed-industries/extensions`.

After that PR is merged, Zed packages and publishes the extension to the registry.

## Notes

This extension does not bundle dbt Fusion. That is intentional: users should install Fusion or configure the language server binary path explicitly.
