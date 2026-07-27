# Postmite

個人のAPI開発作業を、軽量なデスクトップGUI上で完結させるためのローカルファーストなAPIクライアント。

## Language

**ローカルワークスペース**:
リクエスト、Collection、Environment、履歴、Cookie Jar、タブ、Draftを一台の端末内にまとめた、個人が所有する作業領域。複数作成できるが互いに暗黙共有せず、アカウントやクラウド上のチーム領域にも紐づかない。
_Avoid_: チームワークスペース、クラウドワークスペース

**リクエスト**:
送信先、HTTPメソッド、Query、Header、Body、認証設定からなる、一回以上実行可能なAPI呼び出しの定義。
_Avoid_: API

**保存済みRequest**:
Collectionに所属し、明示的なSave操作によって内容が確定されたリクエスト。
_Avoid_: Request Draft、実行記録

**Request Draft**:
タブで編集中のリクエスト状態。クラッシュ復旧のため自動保存されるが、明示的にSaveされるまで保存済みRequestを変更しない。
_Avoid_: 保存済みRequest

**レスポンスプレビュー**:
結果表示と履歴確認のために保持する、サイズを制限したレスポンス本文。ダウンロードされたレスポンス全体とは区別する。
_Avoid_: 完全なレスポンス、ダウンロードファイル

**Bodyファイル**:
multipartまたはBinary Bodyから参照され、通常はワークスペースへコピーされないローカルファイル。Base Directory配下では相対Path、それ以外では非可搬な絶対Pathを持つ。
_Avoid_: 添付済みファイル、レスポンスダウンロード

**実行記録**:
Secretを除いた送信時のリクエスト定義とレスポンス概要を保持し、現在のEnvironmentで再実行できる履歴項目。通信を完全に再現する監査ログではない。
_Avoid_: 通信ログ、保存済みRequest

**Collection**:
再利用するリクエストを階層的に整理し、保存する単位。
_Avoid_: プロジェクト、フォルダー

**Environment**:
リクエスト内で参照する変数の名前と値を、接続先などの利用状況ごとにまとめた切り替え可能な集合。
_Avoid_: 設定、プロファイル

**変数**:
`{{variable_name}}`としてリクエストから参照され、Collectionまたは選択中のEnvironmentに一つの値を持つ名前付きデータ。同名の場合はEnvironmentの値がCollectionの値より優先される。
_Avoid_: Global変数、Local変数、初期値、現在値

**Secret**:
認証情報やトークンなど、通常データとは分離して保護され、表示・コピー・Exportに明示操作を要する値。
_Avoid_: 通常変数、平文の資格情報

**Cookie Jar**:
一つのローカルワークスペース内で、接続先の規則に従ってCookieを保存しリクエスト間で共有する容器。Session Cookieはアプリ終了まで、永続Cookieは有効期限まで保持される。
_Avoid_: Environment、Cookie Header

**ネイティブバックアップ**:
ローカルワークスペースに属する情報を、アプリ固有の情報も失わずに退避・復元できる可搬形式。
_Avoid_: Postman Export

**Import記録**:
外部データの出所、取り込んだ時点の内容、未対応項目、警告を保持し、同じデータの再Import時に差分判定するための記録。
_Avoid_: Collection、ネイティブバックアップ
