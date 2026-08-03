# Postmite

Postmiteは、個人のAPI開発作業を端末内で完結させる、Tauri製のデスクトップAPIクライアントです。

v0.2.0はUbuntu 24.04 LTS x86_64、Windows x64、Apple Silicon macOS向けのプレビューです。
ローカルワークスペースを中心に、HTTPリクエストの作成と実行、Collection、Environment、実行記録を一つのアプリで扱えます。
v0.2.0は、公開済みのv0.1.1を変更せず、Windows x64とApple Silicon macOSの配布対象を追加したリリースです。

## 主な機能

- QueryとHeaderの順序および重複を維持したリクエスト編集
- raw、フォーム、multipart、Bodyファイルを使ったリクエスト実行とキャンセル
- Basic、Bearer、API Key、OAuth 2.0認証とCookie Jar
- Collection、Environment、変数、保存済みRequest、Request Draft、実行記録のローカル保存
- レスポンスプレビュー、大きなレスポンスの段階表示、レスポンスファイルの保存
- Postman Collection v2.1とcURLのImport、Postman形式のExport、ネイティブバックアップと復元
- 診断BundleのExport、SQLite破損時の復旧支援、日本語と英語の表示

アカウント、クラウド同期、チーム共有、GraphQL、WebSocket、pre-request script、test script、OpenAPIはv0.2.0の対象外です。

## インストール

[GitHub Releases](https://github.com/sino-s/postmite/releases)から、対象プラットフォームのパッケージと`SHA256SUMS`を同じディレクトリへダウンロードしてください。

- `Postmite_0.2.0_amd64.deb`または`Postmite_0.2.0_amd64.AppImage`と`linux-x86_64-SHA256SUMS`（Ubuntu）
- `Postmite_0.2.0_x64_en-US.msi`と`windows-x86_64-SHA256SUMS`（Windows x64）
- `Postmite_0.2.0_aarch64.dmg`と`macos-aarch64-SHA256SUMS`（Apple Silicon macOS）

配布パッケージには署名がありません。
インストールまたは実行の前に、使用するパッケージのSHA-256を検証してください。

```bash
sha256sum --check linux-x86_64-SHA256SUMS  # Ubuntu
sha256sum --check windows-x86_64-SHA256SUMS  # Windows x64
sha256sum --check macos-aarch64-SHA256SUMS  # Apple Silicon macOS
```

使用するパッケージが`OK`になったことを確認してから実行します。

### Debian package

```bash
sudo apt install ./Postmite_0.2.0_amd64.deb
postmite
```

インストール後は、デスクトップのアプリケーション一覧からもPostmiteを起動できます。

### AppImage

```bash
chmod +x ./Postmite_0.2.0_amd64.AppImage
./Postmite_0.2.0_amd64.AppImage
```

AppImageはシステムへインストールせずに実行できます。

### Windows x64

ダウンロードした`Postmite_0.2.0_x64_en-US.msi`を実行してインストールします。Windowsパッケージは署名されていません。

### Apple Silicon macOS

ダウンロードした`Postmite_0.2.0_aarch64.dmg`を開き、表示されたアプリケーションを使用します。macOSパッケージは署名されていません。

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
WindowsとmacOSのProtected valuesはsession-only・memory-onlyで、ネイティブのCredential ManagerやKeychainへ永続化しません。

## 更新

Postmiteはバックグラウンドで更新を確認しません。
ユーザーが画面上の**Check for updates**を選んだときだけGitHub Releaseを確認します。
v0.2.0に自動更新機能はありません。

変更内容は[Release notes](./release/RELEASE_NOTES.md)で確認できます。
不具合や要望は[GitHub Issues](https://github.com/sino-s/postmite/issues)へ報告してください。

## Development

開発環境はUbuntu 24.04 LTS x86_64、Node.js 22、pnpm 11.1.1、Rust 1.88.0を基準とします。
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

### Release performance

`pnpm perf:release`は、Tauri release binaryの起動時間、実行ファイルサイズ、WebKitGTKプロセスツリーのメモリを測定します。
Linuxのメモリ判定には、共有ページを重複計上しにくいPSSを使用し、RSSは診断値として残します。
PSSを取得できない環境では、同じシナリオのRSSを代替値として明示します。
実行ファイルサイズは、`src-tauri/target/release/postmite`のstrip前のELFファイルを1024の累乗でMiBへ換算した値です。
Rust compilerの更新はcodegen結果とサイズを変えるため、repositoryのRust 1.88.0 pinを維持し、更新時はpackage-size baselineを再計測します。

one-tabとten-tabはそれぞれ3回測定し、起動順の偏りを抑えるため交互に実行して、各指標の中央値を採用します。
JSONには全raw sampleと中央値を含めます。
2026-07-31のGitHub-hosted Ubuntu 24.04におけるmainの5実行（Actions run 30646691463、30647728120、30650635334、30657751393、30658653032）では、one-tab PSSが259.15〜276.85 MiB、ten-tab PSSが162.72〜267.20 MiBでした。
両シナリオの予算は、観測最大値に約8〜12%のheadroomを加えた300 MiBです。

通常の`pnpm perf:release`は予算超過を報告しますが終了コードを失敗にしません。
`pnpm perf:release:strict`は、one-tab PSS、ten-tab PSS、起動時間、実行ファイルサイズのいずれかが予算を超えると失敗します。
CIのrelease performance jobはmain、手動実行、tagでのみ動作し、計測の安定性を確認する間は非strictです。
headless Linuxでは`dbus-run-session -- xvfb-run -a pnpm perf:release`を使用します。
出力のhost sessionと、測定対象へ実際に渡した`GDK_BACKEND`および`WEBKIT_DISABLE_DMABUF_RENDERER`は別々に記録されます。

実装はGitHub Issueを起点に、1 Issue、1 Plan、1 Commit、1 Pull Requestで進めます。
ContributorとAI Agentは、作業前に[Agent Workflow](./AGENTS.md)と[Domain Language](./CONTEXT.md)を確認してください。
リリース担当者は[Release procedure](./release/RELEASING.md)に従ってください。

## License

Postmiteは[Apache License 2.0](./LICENSE)で公開します。
