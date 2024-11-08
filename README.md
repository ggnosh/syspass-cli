[![Run tests](https://github.com/ggnosh/syspass-cli/actions/workflows/rust.yml/badge.svg)](https://github.com/ggnosh/syspass-cli/actions/workflows/rust.yml)

# syspass-cli

A simple command line tool to interact with sysPass databases.

[sysPass](https://www.syspass.org/) Intuitive, secure and multiuser password manager

## Features

- Supports sysPass 2.1 and 3.2
    - 2.1 has limited functionality because the API doesn't support all the features such as but not limited to:
        - Changing passwords
- Search for accounts and view their passwords
- Add new entries and change passwords from the commandline
- Add new categories from the commandline
- Add new clients from the commandline

## Installation

### From source

```sh
git clone https://github.com/ggnosh/syspass-cli.git
cd syspass-cli
cargo build --release
# cp target/release/syspass-cli somewhere
```

### Cargo

If you already have a Rust environment set up, you can use the `cargo install` command:

```sh
cargo install syspass-cli
```

### From release

Download binary from https://github.com/ggnosh/syspass-cli/releases

## Configuration

**syspass-cli** will look for a configuration file in `$(HOME)/.syspass/config.json`

**NOTE**
The password can be set in plaintext in the `config.json` file or as an environment variable.
If no `SYSPASS_PASSWORD` is found, **syspass-cli** will prompt for it.

### Config file

Create a config file at `$(HOME)/.syspass/config.json`

```json
{
  "host": "https://example.org/api.php",
  "token": "AUTHORIZATION_TOKEN",
  "password": "PASSWORD",
  "verifyHost": true,
  "passwordTimeout": 15,
  "apiVersion": "SyspassV3",
  "noShell": false,
  "noClipboard": false
}
```

`noShell` implies `--no-shell` when searching
`noClipboard` implies `--show-password` when searching and doesn't try to copy things to the clipboard.

If `password` is empty it will be prompted when needed.

`passwordTimeout` if the value is 0 this feature is ignored.
Otherwise, the clipboard will be cleared after given seconds unless the `--showpassword` flag is given.

`apiVersion` defines which API to use. Supported values are `SyspassV2` and `SyspassV3`.
If value is not defined the **syspass-cli** defaults to newest sysPass version.

### Usage file

Located at `$(HOME)/.syspass/usage.json`

This file is used to sort the most commonly used accounts.
The behaviour can be disabled by using `-u` or `--disableusage` during account search.

## Usage:

```text
Usage: syspass-cli [OPTIONS] [COMMAND]

Commands:
  search, -s    Search for account password [aliases: find]
  edit, -e      Edit entity [aliases: change]
  remove, -r    Remove entity [aliases: delete]
  new, -n       Add a new entity [aliases: add]
  check-update  Update syspass-cli
  help          Print this message or the help of the given subcommand(s)

Options:
  -c, --config <FILE>              Sets a custom config file
  -d, --debug                      Output debug information
  -q, --quiet                      Do not output any message
  -v, --verbose                    Output more information
      --completions <completions>  Output debug information [possible values: bash, elvish, fish, powershell, zsh]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Completions

* Shell completions for relevant shells, by invoking `syspass-cli --completions` after building, e.g.

```sh
syspass-cli --completions zsh | sudo tee /usr/share/zsh/site-functions/_syspass-cli > /dev/null
```

For development see [DEVELOPMENT.md](DEVELOPMENT.md)
