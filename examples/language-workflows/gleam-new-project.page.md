# Gleam new project

> Create and maintain a Gleam package using the built-in build tool.
> Official documentation: <https://gleam.run/writing-gleam/>.

- Create a project and enter it:

`gleam new {{project}} && cd {{project}}`

- Run and test the project:

`gleam run && gleam test`

- Format sources and verify formatting in CI:

`gleam format src test && gleam format --check src test`

- Add a Hex package:

`gleam add {{package}}`

- Resolve newer dependency versions:

`gleam update`
