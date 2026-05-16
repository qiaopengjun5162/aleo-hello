# Contributing

Thanks for your interest! This is a minimal demo project, but contributions that help fellow Aleo developers are welcome.

## Ways to contribute

- **Add more program examples** — new Leo programs with different data types or logic
- **Improve documentation** — better explanations, translations, or additional troubleshooting cases
- **Fix issues** — bugs or SDK compatibility problems
- **Share your experience** — if you hit a new problem, add it to `TROUBLESHOOTING.md`

## Setup

```bash
# Leo program
leo build

# TypeScript client
cd client-ts
pnpm install
pnpm exec tsc --noEmit   # type check
```

## Before submitting

- TypeScript client must pass type check
- Update relevant docs if adding new features or troubleshooting cases
- Keep PRs focused and small

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
