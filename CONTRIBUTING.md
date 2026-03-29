# Contributing to DDL

Thank you for your interest in contributing to DDL!

## Development Setup

1. Clone the repository:
   ```bash
   git clone https://github.com/nrj5k/DumbDistributedLog.git
   cd DumbDistributedLog
   ```

2. Install Rust (1.70+):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. Build:
   ```bash
   cargo build
   ```

4. Run tests:
   ```bash
   cargo test
   ```

## Code Style

- Follow Rust API guidelines
- Use `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Write tests for new features

## Pull Request Process

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes
4. Add tests for your changes
5. Ensure tests pass (`cargo test`)
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

## Commit Messages

- Use clear, descriptive commit messages
- Reference issues: "Fixes #123" or "Related to #456"

## License

By contributing, you agree that your contributions will be licensed under the GPL-3.0 License.