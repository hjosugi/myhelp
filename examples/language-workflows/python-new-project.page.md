# Python new project

> Create and maintain a Python application or package with uv.
> Official documentation: <https://docs.astral.sh/uv/concepts/projects/init/> and <https://docs.astral.sh/uv/reference/cli/#uv-init>.

- Create a packaged project without asking uv to download Python:

`uv init --package --no-python-downloads --no-pin-python {{project}} && cd {{project}}`

- Add runtime and development dependencies:

`uv add {{package}} && uv add --dev pytest ruff`

- Run the application or test suite in the locked environment:

`uv run {{project}} && uv run pytest`

- Format, lint, and inspect the dependency tree:

`uv run ruff format . && uv run ruff check . && uv tree`

- Upgrade the lockfile and environment, then review the `uv.lock` diff:

`uv lock --upgrade && uv sync`

- Enter the reproducible shell first when the project also has a Nix Flake:

`nix develop`

- Override a declared no-op personal overlay without committing a machine path:

`nix develop --override-input personal {{personal_flake_url}}`
