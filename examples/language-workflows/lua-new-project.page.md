# Lua new project

> Start a small Lua program using the standard interpreter.
> Official documentation: <https://www.lua.org/start.html> and <https://www.lua.org/manual/>.

- Create a minimal source and test layout:

`mkdir -p {{project}}/src {{project}}/test && cd {{project}}`

- Run a program:

`lua src/main.lua`

- Check that a file parses without running it:

`luac -p src/main.lua`

- Open the interactive interpreter:

`lua`

- Show the exact language version before consulting its matching manual:

`lua -v`
