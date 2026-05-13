# Testing Conventions

## Project test layout

```
tests/
├── setup.ts              # Global test setup (Tauri mocks, reset between tests)
├── mocks/
│   └── tauri.ts          # Mock invoke/listen with settable handlers
└── unit/
    ├── explorer/         # File/tab management tests
    ├── ai/               # AI chat, edit parser tests
    ├── knowledge/        # Knowledge module tests
    └── git/              # Git utilities tests

src-tauri/
├── src/modules/*/mod.rs  # Inline #[cfg(test)] mod tests blocks
└── tests/                # Integration tests (future)
```

## How to mock `invoke` in frontend tests

```ts
import { mockInvoke, expectInvoked } from '../mocks/tauri';

// Register a handler before the code under test runs
mockInvoke('read_file_content', (args) => {
  return 'file contents here';
});

// After calling the code under test:
expectInvoked('read_file_content', { path: '/some/path' });
```

Unregistered commands throw by default — tests don't silently pass on mock-out errors.

## How to use the temp-project fixture in Rust tests

```rust
use tempfile::TempDir;

fn make_temp_project() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().to_path_buf();
    (dir, path)
}
```

## Conventions

- One assertion per test name (test name describes what's being asserted)
- AAA pattern: Arrange, Act, Assert
- No shared mutable state across tests
- Use `beforeEach` for setup, not module-level state

## Running tests

```bash
# Full suite (Rust + frontend + type-check)
npm test

# Frontend only
npx vitest run

# Single frontend test
npx vitest run --reporter=verbose -t "test name"

# Rust only
cd src-tauri && cargo test

# Single Rust test
cd src-tauri && cargo test test_name
```
