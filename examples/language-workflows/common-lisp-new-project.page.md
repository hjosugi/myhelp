# Common Lisp new project

> Create an ASDF project with SBCL, Quicklisp, and quickproject.
> Official documentation: <https://www.quicklisp.org/beta/> and <https://www.quicklisp.org/beta/UNOFFICIAL/docs/quickproject/readme.html>.

- Open the SBCL REPL:

`sbcl`

- Load quickproject in the REPL:

`(ql:quickload :quickproject)`

- Create a project skeleton in Quicklisp's local-projects directory:

`(quickproject:make-project #p"~/quicklisp/local-projects/{{project}}/")`

- Load the local system:

`(ql:quickload :{{project}})`

- Run its ASDF tests:

`(asdf:test-system :{{project}})`
