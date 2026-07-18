# Haskell new project

> Create and maintain a Haskell application with Cabal.
> Official documentation: <https://cabal.readthedocs.io/en/stable/getting-started.html>.

- Create a project interactively:

`cabal init {{project}} && cd {{project}}`

- Accept defaults for a quick non-interactive application:

`cabal init {{project}} --non-interactive && cd {{project}}`

- Build, run, and test:

`cabal build && cabal run && cabal test`

- Open a REPL with the project loaded:

`cabal repl`

- Refresh the package index before reviewing dependency bounds:

`cabal update`
