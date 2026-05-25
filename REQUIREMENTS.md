# zed-dbt-extension requirements

Date: 2026-05-24

## Goal

Build a Zed extension that makes dbt projects easier to edit by providing language recognition, syntax highlighting, snippets, and eventually dbt-aware language server integration.

## Current context

This repository is empty. A Zed extension is expected to be a Git repository with an `extension.toml` manifest at the root. If the extension needs procedural behavior, it should include Rust code compiled to WebAssembly through `zed_extension_api`.

Official Zed docs checked:

- https://zed.dev/docs/extensions/developing-extensions
- https://zed.dev/docs/extensions/languages
- https://docs.rs/zed_extension_api/latest/zed_extension_api/

## MVP scope

The first usable version should focus on editor support that can be shipped without building a full dbt language server.

- `extension.toml`
  - Unique extension id, likely `dbt`
  - Name, version, authors, description, repository
  - Language registrations
  - Grammar registrations if custom tree-sitter grammars are used
- Language definitions
  - `dbt SQL` for dbt model SQL files
  - `dbt YAML` or improved YAML handling for dbt project and schema files
  - Optional `dbt Markdown` for docs blocks later
- File matching
  - `*.sql` inside dbt project directories
  - `dbt_project.yml`
  - `schema.yml`, `sources.yml`, `metrics.yml`, `semantic_models.yml`, `exposures.yml`
  - `macros/**/*.sql`, `models/**/*.sql`, `snapshots/**/*.sql`, `tests/**/*.sql`
- Syntax highlighting
  - SQL highlighting
  - Jinja delimiters: `{{ ... }}`, `{% ... %}`, `{# ... #}`
  - Common dbt functions: `ref`, `source`, `config`, `var`, `env_var`, `doc`, `adapter`, `this`
  - Macro definitions and calls
- Snippets
  - `ref`
  - `source`
  - `config`
  - `macro`
  - `test`
  - Common model materializations
- Local development instructions
  - Install Rust via `rustup`
  - Install the extension in Zed with `zed: install dev extension`
  - Check `zed: open log` when debugging

## Later scope

These features require more design or external tooling.

- Language server integration
  - Start by checking whether an existing dbt language server can be invoked reliably.
  - If using a server, the extension should download it or use a user-installed binary instead of bundling it.
  - Map Zed languages to the LSP language ids expected by the server.
- dbt project awareness
  - Detect workspace root by locating `dbt_project.yml`.
  - Read dbt project paths from `dbt_project.yml`.
  - Respect non-standard model, macro, seed, snapshot, and test paths.
- Go to definition
  - `ref("model")` to model SQL file
  - `source("source", "table")` to source YAML definition
  - Macro call to macro definition
- Completion
  - Model names
  - Source names and tables
  - Macro names
  - Variables from project config where practical
- Diagnostics
  - Surface parse errors if a language server provides them.
  - Avoid trying to validate warehouse-specific SQL in the extension itself.
- Commands and tasks
  - Run `dbt compile`
  - Run `dbt build --select current_model`
  - Run `dbt test --select current_model`
  - Preview compiled SQL if Zed extension APIs support the workflow cleanly.

## Out of scope for the first version

- Full SQL dialect validation
- Query execution and result grids
- Lineage graph UI
- dbt Cloud authentication
- Reimplementing dbt parsing from scratch
- Porting the entire VS Code dbt Power User feature set

## Technical decisions to make

- Whether to use an existing tree-sitter grammar for SQL plus Jinja injections, or maintain a dbt-specific grammar.
- Whether `*.sql` should be globally associated with dbt SQL or only within dbt projects.
- Whether the first release should include Rust/Wasm code, or remain manifest/language/snippet only.
- Which language server, if any, is suitable and legally distributable.
- Minimum Zed version to support.
- License. Zed extension publishing requires an accepted license such as MIT or Apache 2.0.

## Suggested repository structure

```text
zed-dbt-extension/
  extension.toml
  LICENSE
  README.md
  languages/
    dbt-sql/
      config.toml
      highlights.scm
      injections.scm
    dbt-yaml/
      config.toml
      highlights.scm
  snippets/
    dbt-sql.json
    dbt-yaml.json
  Cargo.toml
  src/
    lib.rs
```

`Cargo.toml` and `src/lib.rs` are only needed once the extension needs procedural behavior such as starting a language server or resolving external binaries.

## Validation checklist

- Zed can install the directory as a dev extension.
- Zed recognizes dbt SQL files with the intended language.
- SQL and Jinja syntax highlight correctly in representative dbt model files.
- YAML files used by dbt still receive useful highlighting.
- Snippets appear in the expected languages.
- The extension has a root license file.
- The extension does not bundle language server binaries.

## Open questions

- Should this extension target personal use first, or publication in the Zed extensions registry?
- Should `*.sql` be treated as dbt SQL by default, or only when a `dbt_project.yml` exists?
- Is the desired baseline "syntax and snippets" or "dbt-aware navigation and completion"?
- Which dbt workflows are most important: modeling, macros, tests, docs, sources, or semantic layer files?
