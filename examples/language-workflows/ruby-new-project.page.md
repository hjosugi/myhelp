# Ruby new project

> Create and maintain a Ruby gem with Bundler.
> Official documentation: <https://bundler.io/guides/creating_gem.html>.

- Generate a gem project:

`bundle gem {{project}} && cd {{project}}`

- Install the locked dependencies:

`bundle install`

- Run the generated test task:

`bundle exec rake`

- Build the gem:

`bundle exec rake build`

- Review outdated dependencies before updating selected gems:

`bundle outdated`
