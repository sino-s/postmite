# Postmite

Postmiteは、個人のAPI開発作業を端末内で完結させる、Tauri製のデスクトップAPIクライアントです。

v0.1.1はUbuntu 24.04 LTS x86_64向けのプレビューです。
ローカルワークスペースを中心に、HTTPリクエストの作成と実行、Collection、Environment、実行記録を一つのアプリで扱えます。
v0.1.1は、公開済みのv0.1.0を変更せず、ワークスペースとEnvironmentの管理機能を追加した修正版です。

## 主な機能

- QueryとHeaderの順序および重複を維持したリクエスト編集
- raw、フォーム、multipart、Bodyファイルを使ったリクエスト実行とキャンセル
- Basic、Bearer、API Key、OAuth 2.0認証とCookie Jar
- Collection、Environment、変数、保存済みRequest、Request Draft、実行記録のローカル保存
- レスポンスプレビュー、大きなレスポンスの段階表示、レスポンスファイルの保存
- Postman Collection v2.1とcURLのImport、Postman形式のExport、ネイティブバックアップと復元
- 診断BundleのExport、SQLite破損時の復旧支援、日本語と英語の表示

アカウント、クラウド同期、チーム共有、GraphQL、WebSocket、pre-request script、test script、OpenAPIはv0.1.1の対象外です。
WindowsとmacOSもv0.1.1のサポート対象ではありません。

## インストール

[GitHub Releases](https://github.com/sino-s/postmite/releases)から、次の3ファイルを同じディレクトリへダウンロードしてください。

- `Postmite_0.1.1_amd64.deb`
- `Postmite_0.1.1_amd64.AppImage`
- `SHA256SUMS`

配布する`.deb`とAppImageには署名がありません。
インストールまたは実行の前に、両方のSHA-256を検証してください。

```bash
sha256sum --check SHA256SUMS
```

両方のファイルが`OK`になったことを確認してから、いずれか一方を使用します。

### Debian package

```bash
sudo apt install ./Postmite_0.1.1_amd64.deb
postmite
```

インストール後は、デスクトップのアプリケーション一覧からもPostmiteを起動できます。

### AppImage

```bash
chmod +x ./Postmite_0.1.1_amd64.AppImage
./Postmite_0.1.1_amd64.AppImage
```

AppImageはシステムへインストールせずに実行できます。

## 最初のリクエスト

初回起動時に選択されているPersonalワークスペースで、HTTPメソッドとURLを入力します。
必要に応じてQuery、Header、Body、認証、Environmentを設定し、Sendを選ぶとレスポンスプレビューと実行記録を確認できます。
繰り返し使うリクエストはCollectionへ保存できます。

## データとSecretの保存先

Postmiteはアカウントやクラウドを使用せず、アプリケーションデータを端末内に保存します。
Linuxでは、ローカルワークスペース、Collection、Environment、実行記録、Cookieのメタデータ、診断ログなどを次のアプリケーションデータディレクトリに保存します。

- `XDG_DATA_HOME`に絶対パスが設定されている場合は、`$XDG_DATA_HOME/io.github.sino-s.postmite/`
- 未設定の場合は、`$HOME/.local/share/io.github.sino-s.postmite/`

主なデータベースは、このディレクトリの`postmite.sqlite3`です。
ウィンドウサイズは、`XDG_CONFIG_HOME`に絶対パスが設定されている場合、`$XDG_CONFIG_HOME/io.github.sino-s.postmite/.window-state.json`へ保存します。
未設定の場合は、`$HOME/.config/io.github.sino-s.postmite/.window-state.json`を使用します。
大きなレスポンスの一時ファイルは`${TMPDIR:-/tmp}/postmite-response-files/`に置き、期限切れのファイルを次回起動時に、残りをウィンドウ終了時に削除します。

ネイティブバックアップ、Postman形式のExport、診断Bundle、保存したレスポンスは、ユーザーが選択した出力先へ書き込みます。
Bodyファイルは元の場所を参照し、通常はアプリケーションデータディレクトリへコピーしません。
Secret値を除き、Postmiteがファイルを作成する場所は、以上のアプリケーションデータ、ウィンドウ状態、一時ファイル、ユーザーが選択した出力先です。

Secret値はSQLite、診断ログ、Export、バックアップへ平文で保存しません。
UbuntuではOSのSecret ServiceへSecret値を保存します。
Secret Serviceが利用できない場合やロックされている場合、その起動中だけ有効なセッションストレージへ切り替わり、アプリ終了後には復元できません。

## 更新

Postmiteはバックグラウンドで更新を確認しません。
ユーザーが画面上の**Check for updates**を選んだときだけGitHub Releaseを確認します。
v0.1.1に自動更新機能はありません。

変更内容は[Release notes](./release/RELEASE_NOTES.md)で確認できます。
不具合や要望は[GitHub Issues](https://github.com/sino-s/postmite/issues)へ報告してください。

## Development

開発環境はUbuntu 24.04 LTS x86_64、Node.js 22、pnpm 11.1.1、stable Rustを基準とします。
Tauriのビルドには`libayatana-appindicator3-dev`、`librsvg2-dev`、`libwebkit2gtk-4.1-dev`が必要です。

```bash
sudo apt update
sudo apt install -y libayatana-appindicator3-dev librsvg2-dev libwebkit2gtk-4.1-dev
pnpm install --frozen-lockfile
pnpm tauri
```

Pull Requestと同じ主要な検証は次のコマンドで実行できます。

```bash
pnpm ci:rust
pnpm ci:frontend
```

実装はGitHub Issueを起点に、1 Issue、1 Plan、1 Commit、1 Pull Requestで進めます。
ContributorとAI Agentは、作業前に[Agent Workflow](./AGENTS.md)と[Domain Language](./CONTEXT.md)を確認してください。
リリース担当者は[Release procedure](./release/RELEASING.md)に従ってください。

## License

Postmiteは[Apache License 2.0](./LICENSE)で公開します。
