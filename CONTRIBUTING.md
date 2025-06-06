# Contributing to WasmSX

Thank you for your interest in contributing to WasmSX! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Please be respectful and constructive in all interactions. We welcome contributors of all experience levels and backgrounds.

## Getting Started

1. Fork the repository on GitHub
2. Clone your fork locally
3. Create a new branch for your feature or bug fix
4. Make your changes
5. Submit a pull request

## Development Setup

### Prerequisites

- Rust (latest stable version)
- Node.js (v16 or later)
- Yarn package manager
- cargo-make: `cargo install cargo-make`
- wasm-pack: `cargo install wasm-pack`

### Building the Project

```bash
# Install dependencies and build
cargo make build

# Run development server with hot reload
cargo make dev

# Run tests
cargo test
```

## Project Structure

- `/src/` - Core emulator in Rust
  - `machine.rs` - Main emulation loop
  - `vdp.rs` - Video processor
  - `psg.rs` - Sound generator
  - `bus.rs` - System bus
  - `keyboard.rs` - Input handling
  
- `/client/` - Web frontend
  - `/src/` - TypeScript source
  - `/pkg/` - WASM build output

- `/tests/` - Integration tests
  - `/fixtures/` - Test data files

## Coding Standards

### Rust Code

- Follow standard Rust naming conventions
- Use `rustfmt` for formatting
- Add documentation comments for public APIs
- Write tests for new functionality
- Keep functions focused and small

### TypeScript Code

- Use TypeScript for all new code
- Follow existing code style
- Add type annotations
- Handle errors appropriately

### Commit Messages

- Use clear, descriptive commit messages
- Start with a verb (Add, Fix, Update, etc.)
- Reference issue numbers when applicable
- Keep the first line under 72 characters

Example:
```
fix: correct sprite rendering in screen mode 2

- Fixed color palette mapping for 16x16 sprites
- Added boundary checks for sprite coordinates
- Resolves #123
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_screen1_color

# Run tests with output
cargo test -- --nocapture
```

### Writing Tests

- Add unit tests for individual components
- Add integration tests for complex features
- Use the fixtures in `/tests/fixtures/`
- Test edge cases and error conditions

## Areas for Contribution

### Current Priorities

- **Screen Mode 2** - Complete implementation
- **Sound emulation** - Improve accuracy
- **Performance** - Optimize critical paths
- **Documentation** - Improve code comments and docs
- **Testing** - Increase test coverage

### Good First Issues

Look for issues labeled "good first issue" on GitHub. These are typically:
- Documentation improvements
- Small bug fixes
- Adding tests
- Code cleanup tasks

## Submitting Changes

### Pull Request Process

1. Ensure all tests pass: `cargo test`
2. Update documentation if needed
3. Add tests for new functionality
4. Ensure no compiler warnings
5. Update CHANGELOG.md if applicable

### Pull Request Template

When creating a PR, please include:
- Description of changes
- Related issue numbers
- Testing performed
- Screenshots (for UI changes)

## Architecture Decisions

### Memory Management

- Use slot-based memory system
- Minimize allocations in hot paths
- Prefer stack allocation where possible

### Timing

- Maintain cycle accuracy
- Use the Clock struct for timing
- Account for all CPU cycles

### Rendering

- Render to canvas efficiently
- Batch draw operations
- Update only changed regions when possible

## Getting Help

- Open an issue for bugs or features
- Ask questions in discussions
- Check existing issues first
- Join our community chat (if available)

## Recognition

Contributors will be recognized in:
- The project README
- Release notes
- The contributors page

Thank you for contributing to WasmSX!