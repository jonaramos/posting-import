# posting-import

[![Crates.io](https://img.shields.io/crates/v/posting-import)](https://crates.io/crates/posting-import)
[![License](https://img.shields.io/crates/l/posting-import)](LICENSE)
[![Build](https://github.com/jonaramos/posting-import/actions/workflows/ci.yml/badge.svg)](https://github.com/jonaramos/posting-import/actions/workflows/ci.yml)

Import API collections from Postman, Insomnia, and Bruno to [Posting TUI](https://posting.sh) format.

## Overview

`posting-import` is a CLI tool that converts API collections from popular HTTP clients into Posting's Git-friendly YAML format. This allows you to migrate your existing API collections to Posting without losing any data.

### Supported Sources

| Source | Formats | Status |
|--------|---------|--------|
| **Postman** | Collection JSON (v2.0/v2.1) | ✅ Stable |
| **Insomnia** | Export JSON (v4/v5) | ✅ Stable |
| **Bruno** | `.bru` files & OpenCollection YAML | ✅ Stable |

## Installation

### Via Homebrew (macOS)

```bash
# Add the tap
brew tap jonaramos/posting-import

# Install
brew install posting-import
```

### Via Cargo (Cross-platform)

```bash
cargo install posting-import
```

### Via Pre-built Binaries

Download the latest release for your platform from the [GitHub Releases](https://github.com/jonaramos/posting-import/releases) page:

- Linux (x86_64): `posting-import-x86_64-unknown-linux-gnu.tar.gz`
- macOS (Intel): `posting-import-x86_64-apple-darwin.tar.gz`
- macOS (Apple Silicon): `posting-import-aarch64-apple-darwin.tar.gz`

### Via Package Managers

| Package Manager | Platform | Command |
|----------------|----------|---------|
| Homebrew | macOS | `brew install jonaramos/tools/posting-import` |
| Pacman (AUR) | Arch Linux | `pacman -S posting-import` |

## Usage

### Basic Usage

```bash
# Import a Postman collection
posting-import --app postman --source collection.json --target ./output

# Import an Insomnia collection
posting-import --app insomnia --source insomnia-export.json --target ./output

# Import a Bruno collection
posting-import --app bruno --source ./my-bruno-collection --target ./output
```

### Command-line Options

```
USAGE:
    posting-import [OPTIONS] --app <APP> --source <SOURCE> --target <TARGET>

OPTIONS:
    -a, --app <APP>           Source application type (postman, insomnia, bruno) [required]
    -s, --source <SOURCE>     Path to the source collection file or directory [required]
    -t, --target <TARGET>     Output directory for the Posting collection [default: .]
    -w, --overwrite           Overwrite existing files
    -v, --verbose             Verbose output (repeat for more verbosity)
    -n, --dry-run             Don't write output (preview)
    -c, --name <NAME>         Collection name (overrides detected name)
        --list-sources        List supported source formats and exit
        --format <FORMAT>     Output format: text, json, yaml [default: text]
    -h, --help                Print help
    -V, --version             Print version
```

### Examples

#### Import from Postman

1. Export your collection from Postman:
   - Open Postman → Select Collection → Click "Export"
   - Choose "Collection v2.1" or "Collection v2.0.0"
   - Save as JSON file

2. Import to Posting format:

```bash
posting-import -a postman -s my-collection.json -t ./posting-collections
```

#### Import from Insomnia

1. Export from Insomnia:
   - Open Insomnia → Select Workspace/Collection
   - Click "Export" → Choose "Insomnia JSON (v4/v5)"
   - Save as JSON file

2. Import to Posting format:

```bash
posting-import -a insomnia -s insomnia-export.json -t ./posting-collections
```

#### Import from Bruno

Bruno collections are stored as directories. You can import either:

```bash
# Import entire collection directory
posting-import -a bruno -s ./bruno/my-api -t ./posting-collections

# Import specific opencollection.yml file
posting-import -a bruno -s ./bruno/my-api/opencollection.yml -t ./posting-collections
```

#### Preview Before Importing

Use `--dry-run` to see what will be imported without writing files:

```bash
posting-import -a postman -s collection.json -t ./output --dry-run
```

#### Overwrite Existing Files

```bash
posting-import -a postman -s collection.json -t ./output --overwrite
```

## Output Format

The tool creates `.posting.yaml` files compatible with the [Posting TUI](https://posting.sh):

```
output-directory/
├── collection-name/
│   ├── README.md
│   ├── .env                  # Environment variables (if variables are used)
│   ├── request-name-1.posting.yaml
│   ├── request-name-2.posting.yaml
│   └── subfolder/
│       ├── request-name-3.posting.yaml
│       └── request-name-4.posting.yaml
```

### Environment Variables

The importer automatically extracts environment variables from your collections and creates environment files:

- **Variable syntax transformation**: All source-specific variable syntax is converted to Posting's `${VARIABLE}` format
  - Postman: `{{variable}}` → `${variable}`
  - Insomnia: `{{ _.variable }}` → `${variable}`
  - Bruno: `{{variable}}` → `${variable}`

**Environment file naming:**
- **With environments** (Insomnia, Bruno): Creates `{name}.env` files (e.g., `dev.env`, `qa.env`, `production.env`)
- **Without environments**: Creates `posting.env` with variable names (fill in values manually)

Example environment files:

```bash
# When environments exist (Insomnia, Bruno):
# dev.env
baseUrl=https://api.example.com
TOKEN=dev-token

# qa.env
baseUrl=https://qa.example.com
TOKEN=qa-token

# When no environments defined:
# posting.env
baseUrl=
TOKEN=
```

### Example Output File

```yaml
name: Get Users
method: GET
url: https://api.example.com/users
headers:
  - name: Accept
    value: application/json
  - name: Authorization
    value: Bearer ${TOKEN}
auth:
  type: bearer_token
  bearer_token:
    token: ${TOKEN}
```

## Configuration

### Shell Completions

Generate shell completions for your shell:

```bash
# Bash
source <(posting-import --completions bash)

# Zsh
source <(posting-import --completions zsh)

# Fish
posting-import --completions fish | source
```

For permanent installation, copy the completion files from the `completions/` directory:

```bash
# Bash
cp completions/posting-import.bash /etc/bash_completion.d/

# Zsh
cp completions/_posting-import /usr/share/zsh/site-functions/

# Fish
cp completions/posting-import.fish /usr/share/fish/vendor_completions.d/
```

## Development

### Prerequisites

- Rust 1.70+ (or Rust 2024 edition)
- Cargo

### Building from Source

```bash
# Clone the repository
git clone https://github.com/jonaramos/posting-import.git
cd posting-import

# Build release
cargo build --release

# Run tests
cargo test

# Install locally
cargo install --path .
```

### Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_name
```

## Architecture

The project follows a plugin-based architecture:

```
src/
├── main.rs           # Application entry point
├── lib.rs            # Library exports
├── cli/              # Command-line interface (clap)
├── core/             # Core domain models
│   └── models.rs     # Request, Collection, Auth types
├── io/               # Input/Output operations
│   └── writer.rs     # Posting YAML format writer
└── plugins/          # Importer plugins
    ├── mod.rs        # Plugin trait and registry
    ├── postman.rs    # Postman importer
    ├── insomnia.rs   # Insomnia importer
    └── bruno.rs      # Bruno importer
```

### Adding a New Importer

To add support for a new source:

1. Create a new module in `src/plugins/` (e.g., `newsource.rs`)
2. Implement the `ImporterPlugin` trait
3. Register the plugin in `plugins/mod.rs`'s `default_registry()`
4. Add tests
5. Update this README

## Contributing

Contributions are welcome! Please read our contributing guidelines before submitting PRs.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the [GNU General Public License v3.0](LICENSE).

## Related Projects

- [Posting](https://github.com/darrenburns/posting) - The modern HTTP client that lives in your terminal
- [Bruno](https://github.com/usebruno/bruno) - Opensource IDE for exploring and testing APIs
- [OpenCollection](https://github.com/opencollection-dev/opencollection) - An open standard for describing collections

## Support

- [GitHub Issues](https://github.com/jonaramos/posting-import/issues) - Bug reports and feature requests
- [Discussions](https://github.com/jonaramos/posting-import/discussions) - Q&A and community discussion

## Acknowledgments

- Thanks to the [Posting](https://posting.sh) team for creating an excellent TUI HTTP client
- Thanks to all contributors who help improve this tool
