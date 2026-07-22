# Elixir new project

> Create and maintain an Elixir project with Mix.
> Official documentation: <https://hexdocs.pm/mix/Mix.Tasks.New.html>.

- Create a regular project:

`mix new {{project}} && cd {{project}}`

- Create an OTP application with a supervision tree:

`mix new {{project}} --sup && cd {{project}}`

- Fetch dependencies, compile, and test:

`mix deps.get && mix compile && mix test`

- Format all configured files and verify formatting in CI:

`mix format && mix format --check-formatted`

- Update dependencies allowed by `mix.exs`, then review the `mix.lock` diff:

`mix deps.update --all`
