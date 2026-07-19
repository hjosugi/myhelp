# CLI behavior contract

The CLI is designed for both an interactive local terminal and quiet use over
SSH, pipes, and scripts. Terminal conveniences must not change the bytes that a
normal pipeline receives.

## Reading and output

`myhelp show <topic>` selects its output from the destination:

| Destination and option | Output |
|---|---|
| Interactive stdout | Wrapped, tldr-style terminal rendering |
| Pipe or redirected file (default color mode) | Original UTF-8 Markdown |
| `--raw` | Original UTF-8 Markdown on every destination |
| `--json` | One JSON page object followed by a newline |

Rendered output replaces embedded terminal control characters before adding
MyHelp's own styling. Raw and JSON output do not reinterpret page content.

`list` and `search` use aligned columns on a terminal and
`<topic><TAB><title>` when piped. Their `--json` form is a deterministic array
of page summaries in core's topic sort order. `path` remains one path followed
by a newline.

Writing into a pipe whose reader exits early is treated as a successful
pipeline termination rather than an application failure.

## Color and paging

Color defaults to `auto`:

- styling is enabled only when stdout is a terminal;
- any non-empty `NO_COLOR` disables styling;
- `--color never` disables it explicitly;
- `--color always` is an explicit override.

The internal pager is considered only when stdin and stdout are terminals. It
opens only when the selected page output exceeds the terminal height, and it works
on Linux, macOS, and Windows. `--no-pager` always writes directly. Piped output
never opens the pager.

Pager keys include arrows or `j`/`k`, Page Up/Page Down, Space, `g`/`G`,
forward search with `/`, and quit with `q`.

## Explicit fuzzy selection

`myhelp pick [query]` is the only interactive selection command. It requires an
interactive stdin and stderr, uses the optional query as its initial filter,
and displays the selected page. Escape cancels.

For shell composition:

```bash
topic="$(myhelp pick --print-topic)" && myhelp show "$topic"
```

The selector renders on stderr so stdout may be captured. `--print-topic`
prints only the selected topic. MyHelp does not copy a command into the shell
buffer and never executes page content.

## Shell completions

Completion scripts are generated from the current `clap` command definition:

```bash
# Bash
mkdir -p ~/.local/share/bash-completion/completions
myhelp completions bash \
  > ~/.local/share/bash-completion/completions/myhelp

# Fish
mkdir -p ~/.config/fish/completions
myhelp completions fish > ~/.config/fish/completions/myhelp.fish

# Zsh (ensure ~/.zfunc is in fpath before compinit)
mkdir -p ~/.zfunc
myhelp completions zsh > ~/.zfunc/_myhelp

# PowerShell (dot-source the generated file from $PROFILE)
myhelp completions powershell > myhelp.ps1

# Elvish
myhelp completions elvish > myhelp.elv
```

The supported generators are Bash, Fish, Zsh, PowerShell, and Elvish. Generated
scripts go to stdout and do not require a vault to exist.

## Editor commands

`VISUAL` has precedence over `EDITOR`. Both may include arguments:

```bash
export VISUAL='code --wait'
export EDITOR='nvim -f'
```

On Unix, the value uses shell-word quoting. On Windows, it uses
`CommandLineToArgvW`-compatible quoting, so an executable path containing spaces
must be quoted:

```powershell
$env:VISUAL = '"C:\Program Files\Microsoft VS Code\bin\code.cmd" --wait'
```

MyHelp parses the value into an executable and arguments, appends the temporary
draft path, and creates the process directly. It does not pass the value to
`sh`, `cmd.exe`, or PowerShell. The temporary-draft and revision-conflict rules
remain the same as the storage contract.

## Exit codes

| Code | Meaning |
|---:|---|
| 0 | Success, including a downstream pipe closing normally |
| 1 | Runtime, filesystem, pager, or unexpected failure |
| 2 | Command-line usage error emitted by `clap` |
| 3 | Page or selectable content not found |
| 4 | Revision conflict; the disk page won |
| 5 | Invalid or unsafe topic/data, or an interactive command without a terminal |
| 6 | Invalid editor configuration or unsuccessful editor process |
| 130 | Interactive selection cancelled |

Machine-readable consumers should use `--raw`, `--json`, or `path` and should
not parse human-facing error text.
