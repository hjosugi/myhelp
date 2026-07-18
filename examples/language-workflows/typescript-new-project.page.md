# Node.js and TypeScript new project

> Create a Node.js package with pnpm and TypeScript.
> Official documentation: <https://pnpm.io/cli/init> and <https://www.typescriptlang.org/docs/handbook/compiler-options.html>.

- Create an ES module package and pin the package manager:

`mkdir {{project}} && cd {{project}} && pnpm init --init-type module --init-package-manager`

- Add TypeScript and Node.js type definitions:

`pnpm add --save-dev typescript @types/node`

- Generate `tsconfig.json`:

`pnpm exec tsc --init`

- Type-check without emitting JavaScript:

`pnpm exec tsc --noEmit`

- Update dependencies within declared ranges:

`pnpm update`

- Review newer versions, including major releases, before opting in:

`pnpm outdated`
