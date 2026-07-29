# Postmite

Postmiteは、個人のAPI開発作業を端末内で完結させる、軽量なTauri製デスクトップAPIクライアントです。

現在は設計完了・実装開始前の段階です。

## Development

実装作業はGitHub Issueを起点とし、1 Issue、1 Plan、1 Commit、1 Pull Requestで進めます。AI Agentを含むContributorは、作業前に[Agent Workflow](./AGENTS.md)を確認してください。

- [Agent Workflow](./AGENTS.md)
- [Domain Language](./CONTEXT.md)
- [Implementation Backlog](https://github.com/sino-s/postmite/issues)

詳細設計はローカルの`docs/`に置き、RepositoryにはCommitしません。実装Issueは、設計文書がないCloneでも作業できるよう自己完結させます。

### Quality, release, and performance checks

Pull request CI runs deterministic quality gates: Rust format, Rust lint, Rust tests, TypeScript checks including IPC drift detection, frontend lint, frontend tests, and the production web build. The same local command set is exposed through pnpm scripts:

```bash
pnpm ci:rust
pnpm ci:frontend
```

Release-only validation runs on `main`, `v*` tags, and manual workflow dispatch. Use these commands locally when an Issue or plan requires package or desktop-build evidence:

```bash
pnpm ci:build
pnpm release:bundle
pnpm release:verify-candidate
```

Release performance budgets are measured from the Tauri release binary:

```bash
pnpm perf:release
```

The command reports budget failures without failing by default. Use `pnpm perf:release:strict` when a reference environment should reject a performance regression.

On a headless Linux machine, run the performance command under Xvfb:

```bash
xvfb-run -a pnpm perf:release
```

## Initial scope

初版はUbuntu 24.04 LTS x86_64を対象に、REST APIのRequest作成・実行、Collection、Environment、履歴、認証、Postman v2.1およびcURLとの相互運用を提供します。アカウント、クラウド同期、チーム共有、GraphQL、WebSocket、スクリプト実行は対象外です。

## License

Apache License 2.0で公開します。
