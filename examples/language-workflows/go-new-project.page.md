# Go new project

> Create and maintain a Go module.
> Official documentation: <https://go.dev/doc/tutorial/create-module>.

- Create a module using its future repository path:

`mkdir {{project}} && cd {{project}} && go mod init github.com/{{owner}}/{{project}}`

- Run all packages and tests:

`go run . && go test ./...`

- Format and statically analyze the module:

`gofmt -w . && go vet ./...`

- Update dependencies, tidy module files, then review both module-file diffs:

`go get -u ./... && go mod tidy`
