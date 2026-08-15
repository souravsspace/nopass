# Architecture Decision Records

Why the codebase is shaped the way it is. Format: [Michael Nygard's ADR](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).

| ADR | Title | Status | Date |
|-----|-------|--------|------|
| [0001](0001-native-messaging-transport.md) | Native messaging over a localhost server | accepted | 2026-08-08 |
| [0002](0002-read-only-extension-v1.md) | The browser extension is read-only in v1 | accepted | 2026-08-08 |
| [0003](0003-shared-extension-source.md) | One extension source, two thin browser targets | accepted | 2026-08-08 |
| [0004](0004-turborepo-beside-cargo.md) | Turborepo and Bun workspaces beside the Cargo workspace | accepted | 2026-08-08 |
| [0005](0005-unlock-via-nopass-agent.md) | The extension unlocks through the existing nopass agent | accepted | 2026-08-08 |
| [0006](0006-create-only-writes-from-the-extension.md) | The extension may create an entry, and only create one | accepted | 2026-08-09 |
| [0007](0007-saving-a-login-from-the-page.md) | A page may offer a login; only the user may name one | accepted | 2026-08-15 |
| [0008](0008-typed-records-in-the-entry-body.md) | An entry says what it is, on a `type:` line | accepted | 2026-08-15 |
| [0009](0009-updating-an-entry-from-the-extension.md) | The extension may rewrite an entry, field by field | accepted | 2026-08-15 |
