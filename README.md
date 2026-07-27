# Postmite

Postmiteは、個人のAPI開発作業を端末内で完結させる、軽量なTauri製デスクトップAPIクライアントです。

現在は設計完了・実装開始前の段階です。

## Development

実装作業はGitHub Issueを起点とし、1 Issue、1 Plan、1 Commit、1 Pull Requestで進めます。AI Agentを含むContributorは、作業前に[Agent Workflow](./AGENTS.md)を確認してください。

- [Agent Workflow](./AGENTS.md)
- [Domain Language](./CONTEXT.md)
- [Implementation Backlog](https://github.com/sino-s/postmite/issues)

詳細設計はローカルの`docs/`に置き、RepositoryにはCommitしません。実装Issueは、設計文書がないCloneでも作業できるよう自己完結させます。

## Initial scope

初版はUbuntu 24.04 LTS x86_64を対象に、REST APIのRequest作成・実行、Collection、Environment、履歴、認証、Postman v2.1およびcURLとの相互運用を提供します。アカウント、クラウド同期、チーム共有、GraphQL、WebSocket、スクリプト実行は対象外です。

## License

Apache License 2.0で公開します。
