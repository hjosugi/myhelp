# Go new project

> Create and maintain a Go module.
> Official documentation: <https://go.dev/doc/tutorial/create-module>.

- Create a module using its future repository path:

`mkdir {{project}} && cd {{project}} && go mod init github.com/{{owner}}/{{project}}`

- Run all packages and tests:

`go run . && go test ./...`

- Format and statically analyze the module:

`gofmt -w . && go vet ./...`

- Update direct and transitive dependencies, then clean the module files:

`go get -u ./... && go mod tidy`
