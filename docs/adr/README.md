# Architecture decision records

Architecture decision records (ADRs) capture contracts that affect the plain
vault format, interoperability, security, or more than one application layer.

<!-- markdownlint-disable MD060 -->

| ADR | Status | Decision |
|---|---|---|
| [0001](0001-page-metadata-sidecars.md) | Accepted | Keep optional page metadata in adjacent YAML sidecars |
| [0002](0002-foreign-format-adapter-levels.md) | Accepted | Use explicit lossy-preview, read-only, or unsupported compatibility levels for foreign formats |

<!-- markdownlint-enable MD060 -->

An accepted ADR describes the contract for later implementation issues. A
change that breaks an accepted contract requires a superseding ADR rather than
silently rewriting this file.
