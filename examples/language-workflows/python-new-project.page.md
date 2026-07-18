# Python new project

> Create and maintain a Python application or package with uv.
> Official documentation: <https://docs.astral.sh/uv/concepts/projects/init/>.

- Create a packaged project without asking uv to download Python:

`uv init --package --no-pin-python {{project}} && cd {{project}}`

- Add runtime and development dependencies:

`uv add {{package}} && uv add --dev pytest ruff`

- Run the application or test suite in the locked environment:

`uv run {{project}} && uv run pytest`

- Format, lint, and inspect the dependency tree:

`uv run ruff format . && uv run ruff check . && uv tree`

- Refresh the lockfile and environment:

`uv lock --upgrade && uv sync`

- Enter the reproducible shell first when the project also has a Nix Flake:

`nix develop`
