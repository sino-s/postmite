import { createContext, useContext, useMemo, useState, type ReactNode } from "react";

export const supportedLocales = ["en", "ja"] as const;
export type AppLocale = (typeof supportedLocales)[number];

const catalogs = {
  en: {
    "app.language": "Language",
    "app.loading": "Loading Postmite",
    "app.unavailable": "Request workspace unavailable",
    "app.retry": "Retry",
    "app.menu": "Application menu",
    "app.diagnostics": "Diagnostics",
    "app.base": "Base",
    "app.relink": "Relink",
    "app.new": "New",
    "app.theme": "Theme",
    "app.theme.light": "Light",
    "app.theme.dark": "Dark",
    "app.theme.system": "System",
    "app.density": "Density",
    "app.density.comfortable": "Comfortable",
    "app.density.compact": "Compact",
    "app.checkUpdates": "Check for updates",
    "app.checkingUpdates": "Checking for updates",
    "app.updateAvailable": "Version {version} is available.",
    "app.upToDate": "Postmite is up to date.",
    "app.updateCheckFailed": "Update checking is currently unavailable.",
    "app.newRequest": "New Request",
    "app.requestEditor": "Request editor",
    "app.folderName": "Folder name",
    "app.newFolder": "New Folder",
    "app.baseDirectory": "Workspace Base Directory",
    "app.storedBodyPath": "Stored Body file path",
    "app.replacementBodyPath": "Replacement absolute file path",
    "app.runningClose": "This request is still running. Cancel it and close the tab?",
    "app.executionFinished": "Execution was already finished.",
    "app.executionEventsUnavailable": "Response event listener is unavailable. Restart Postmite and try again.",
    "app.executionIdle": "Ready to send request.",
    "app.executionQueued": "Request queued.",
    "app.executionRunning": "Request is running.",
    "app.executionCompleted": "Request completed.",
    "app.executionFailed": "Request failed.",
    "app.executionCancelled": "Request cancelled.",
    "collections.title": "Collections",
    "collections.newRoot": "New root folder",
    "collections.environment": "Environment",
    "collections.noEnvironment": "No environment",
    "collections.noRequests": "No saved requests",
    "collections.tree": "Collection tree",
    "request.name": "Name",
    "request.namePlaceholder": "Request name",
    "request.method": "Method",
    "request.url": "URL",
    "request.save": "Save",
    "request.send": "Send",
    "request.cancel": "Cancel",
    "request.tabs": "Request tabs",
    "request.close": "Close {title}",
    "fields.add": "Add",
    "fields.on": "On",
    "fields.name": "Name",
    "fields.value": "Value",
    "fields.actions": "Actions",
    "fields.remove": "Remove {legend} row {index}",
    "fields.enabled": "{legend} row {index} enabled",
    "fields.none": "No {legend}",
    "body.title": "Body",
    "body.mode": "Body mode",
    "body.urlEncoded": "URL-encoded Body",
    "body.multipartFields": "Multipart Fields",
    "body.multipartFiles": "Multipart Files",
    "body.file": "File",
    "body.refresh": "Refresh",
    "body.raw": "Raw Body",
    "body.rawEditor": "Raw body editor",
    "body.rawPlaceholder": "Raw body",
    "response.title": "Response",
    "response.empty": "No response yet.",
    "response.status": "Status {status}",
    "response.time": "Time {value}",
    "response.timing": "Timing {value}",
    "response.received": "Received {value}",
    "response.decoded": "Decoded {value}",
    "response.wire": "Wire {value}",
    "response.sent": "Sent {value}",
    "response.headers": "Headers",
    "response.details": "Response details",
    "response.noHeaders": "No response headers",
    "response.noBody": "No response body",
    "response.search": "Search response",
    "response.truncated": "Response preview truncated.",
    "response.saved": "Saved {value} bytes to {destination}",
    "history.title": "Execution history",
    "history.loading": "Loading",
    "cookies.title": "Cookie jar",
    "cookies.loading": "Loading",
    "cookies.none": "No cookies",
    "variables.title": "Variables",
    "variables.resolving": "Resolving",
    "security.title": "Security",
    "diagnostics.title": "Diagnostics",
    "diagnostics.close": "Close diagnostics",
    "actions.newSubfolder": "New subfolder",
    "actions.renameFolder": "Rename folder",
    "actions.moveFolderUp": "Move folder up",
    "actions.moveFolderDown": "Move folder down",
    "actions.duplicateFolder": "Duplicate folder",
    "actions.deleteFolder": "Delete folder",
    "actions.duplicateRequest": "Duplicate request",
    "actions.deleteRequest": "Delete request",
    "errors.STATE_UNAVAILABLE": "The application state is unavailable. Try again.",
    "errors.INVALID_INPUT": "The supplied input is invalid.",
    "errors.WORKSPACE_NOT_FOUND": "The workspace was not found.",
    "errors.WORKSPACE_ALREADY_EXISTS": "A workspace with that name already exists.",
    "errors.CANNOT_DELETE_LAST_WORKSPACE": "The last workspace cannot be deleted.",
    "errors.REQUEST_NOT_FOUND": "The request was not found.",
    "errors.SAVED_REQUEST_ALREADY_OPEN": "That saved request is already open.",
    "errors.PERSISTENCE_UNAVAILABLE": "Local storage is currently unavailable. Try again.",
    "errors.DEFAULT": "The operation could not be completed. Try again.",
  },
  ja: {
    "app.language": "言語",
    "app.loading": "Postmite を読み込み中",
    "app.unavailable": "リクエストのワークスペースを利用できません",
    "app.retry": "再試行",
    "app.menu": "アプリケーションメニュー",
    "app.diagnostics": "診断",
    "app.base": "ベース",
    "app.relink": "再リンク",
    "app.new": "新規",
    "app.theme": "テーマ",
    "app.theme.light": "ライト",
    "app.theme.dark": "ダーク",
    "app.theme.system": "システム",
    "app.density": "表示密度",
    "app.density.comfortable": "標準",
    "app.density.compact": "コンパクト",
    "app.checkUpdates": "アップデートを確認",
    "app.checkingUpdates": "アップデートを確認中",
    "app.updateAvailable": "バージョン {version} を利用できます。",
    "app.upToDate": "Postmite は最新です。",
    "app.updateCheckFailed": "アップデートを現在確認できません。",
    "app.newRequest": "新しいリクエスト",
    "app.requestEditor": "リクエストエディター",
    "app.folderName": "フォルダー名",
    "app.newFolder": "新しいフォルダー",
    "app.baseDirectory": "ワークスペースのベースディレクトリ",
    "app.storedBodyPath": "保存済み Body ファイルのパス",
    "app.replacementBodyPath": "置換する絶対ファイルパス",
    "app.runningClose": "このリクエストは実行中です。キャンセルしてタブを閉じますか？",
    "app.executionFinished": "実行はすでに完了しています。",
    "app.executionEventsUnavailable": "レスポンスイベントを受信できません。Postmite を再起動して再試行してください。",
    "app.executionIdle": "リクエストを送信できます。",
    "app.executionQueued": "リクエストを待機に入れました。",
    "app.executionRunning": "リクエストを実行中です。",
    "app.executionCompleted": "リクエストが完了しました。",
    "app.executionFailed": "リクエストに失敗しました。",
    "app.executionCancelled": "リクエストをキャンセルしました。",
    "collections.title": "コレクション",
    "collections.newRoot": "ルートフォルダーを作成",
    "collections.environment": "環境",
    "collections.noEnvironment": "環境なし",
    "collections.noRequests": "保存済みリクエストはありません",
    "collections.tree": "コレクションツリー",
    "request.name": "名前",
    "request.namePlaceholder": "リクエスト名",
    "request.method": "メソッド",
    "request.url": "URL",
    "request.save": "保存",
    "request.send": "送信",
    "request.cancel": "キャンセル",
    "request.tabs": "リクエストタブ",
    "request.close": "{title} を閉じる",
    "fields.add": "追加",
    "fields.on": "有効",
    "fields.name": "名前",
    "fields.value": "値",
    "fields.actions": "操作",
    "fields.remove": "{legend} の {index} 行目を削除",
    "fields.enabled": "{legend} の {index} 行目を有効化",
    "fields.none": "{legend} はありません",
    "body.title": "Body",
    "body.mode": "Body の形式",
    "body.urlEncoded": "URL エンコード Body",
    "body.multipartFields": "マルチパートフィールド",
    "body.multipartFiles": "マルチパートファイル",
    "body.file": "ファイル",
    "body.refresh": "更新",
    "body.raw": "Raw Body",
    "body.rawEditor": "Raw Body エディター",
    "body.rawPlaceholder": "Raw Body",
    "response.title": "レスポンス",
    "response.empty": "レスポンスはまだありません。",
    "response.status": "ステータス {status}",
    "response.time": "時間 {value}",
    "response.timing": "タイミング {value}",
    "response.received": "受信 {value}",
    "response.decoded": "展開後 {value}",
    "response.wire": "通信量 {value}",
    "response.sent": "送信 {value}",
    "response.headers": "ヘッダー",
    "response.details": "レスポンス詳細",
    "response.noHeaders": "レスポンスヘッダーはありません",
    "response.noBody": "レスポンス本文はありません",
    "response.search": "レスポンスを検索",
    "response.truncated": "レスポンスプレビューは省略されています。",
    "response.saved": "{value} バイトを {destination} に保存しました",
    "history.title": "実行履歴",
    "history.loading": "読み込み中",
    "cookies.title": "Cookie Jar",
    "cookies.loading": "読み込み中",
    "cookies.none": "Cookie はありません",
    "variables.title": "変数",
    "variables.resolving": "解決中",
    "security.title": "セキュリティ",
    "diagnostics.title": "診断",
    "diagnostics.close": "診断を閉じる",
    "actions.newSubfolder": "サブフォルダーを作成",
    "actions.renameFolder": "フォルダー名を変更",
    "actions.moveFolderUp": "フォルダーを上へ移動",
    "actions.moveFolderDown": "フォルダーを下へ移動",
    "actions.duplicateFolder": "フォルダーを複製",
    "actions.deleteFolder": "フォルダーを削除",
    "actions.duplicateRequest": "リクエストを複製",
    "actions.deleteRequest": "リクエストを削除",
    "errors.STATE_UNAVAILABLE": "アプリケーションの状態を利用できません。再試行してください。",
    "errors.INVALID_INPUT": "入力内容が正しくありません。",
    "errors.WORKSPACE_NOT_FOUND": "ワークスペースが見つかりません。",
    "errors.WORKSPACE_ALREADY_EXISTS": "同じ名前のワークスペースがすでにあります。",
    "errors.CANNOT_DELETE_LAST_WORKSPACE": "最後のワークスペースは削除できません。",
    "errors.REQUEST_NOT_FOUND": "リクエストが見つかりません。",
    "errors.SAVED_REQUEST_ALREADY_OPEN": "この保存済みリクエストはすでに開かれています。",
    "errors.PERSISTENCE_UNAVAILABLE": "ローカルストレージを利用できません。再試行してください。",
    "errors.DEFAULT": "操作を完了できませんでした。再試行してください。",
  },
} as const;

export type TranslationKey = keyof typeof catalogs.en;
type TranslationValues = Record<string, string | number | bigint>;

export function detectLocale(language = typeof navigator === "undefined" ? "en" : navigator.language): AppLocale {
  return language.toLowerCase().startsWith("ja") ? "ja" : "en";
}

function interpolate(message: string, values?: TranslationValues) {
  return message.replace(/\{(\w+)\}/g, (_, key: string) => String(values?.[key] ?? `{${key}}`));
}

type I18n = {
  locale: AppLocale;
  setLocale: (locale: AppLocale) => void;
  t: (key: TranslationKey, values?: TranslationValues) => string;
  formatDate: (value: number | Date) => string;
  formatNumber: (value: number | bigint) => string;
  formatBytes: (value: number | bigint) => string;
  formatError: (error: unknown) => string;
};

const I18nContext = createContext<I18n | null>(null);

const fallbackI18n: I18n = {
  locale: "en",
  setLocale: () => undefined,
  t: (key, values) => interpolate(catalogs.en[key], values),
  formatDate: (date) => new Intl.DateTimeFormat("en", { dateStyle: "medium", timeStyle: "short" }).format(date),
  formatNumber: (number) => String(number),
  formatBytes: (value) => `${typeof value === "bigint" ? value.toString() : new Intl.NumberFormat("en").format(value)} B`,
  formatError: () => catalogs.en["errors.DEFAULT"],
};

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocale] = useState<AppLocale>(detectLocale);
  const value = useMemo<I18n>(() => ({
    locale,
    setLocale,
    t: (key, values) => interpolate(catalogs[locale][key], values),
    formatDate: (date) => new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(date),
    formatNumber: (number) => new Intl.NumberFormat(locale).format(number),
    formatBytes: (value) => {
      if (typeof value === "bigint" && (value > BigInt(Number.MAX_SAFE_INTEGER) || value < BigInt(Number.MIN_SAFE_INTEGER))) {
        return `${value.toString()} B`;
      }
      const bytes = Number(value);
      if (!Number.isFinite(bytes) || bytes < 1024) return `${new Intl.NumberFormat(locale).format(value)} B`;
      const units = ["KiB", "MiB", "GiB"];
      const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length);
      return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(bytes / 1024 ** unit)} ${units[unit - 1]}`;
    },
    formatError: (error) => {
      const code = typeof error === "object" && error !== null && "code" in error && typeof error.code === "string" ? error.code : "DEFAULT";
      const key = `errors.${code}` as TranslationKey;
      return key in catalogs.en ? interpolate(catalogs[locale][key], undefined) : catalogs[locale]["errors.DEFAULT"];
    },
  }), [locale]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const context = useContext(I18nContext);
  return context ?? fallbackI18n;
}

export function translationKeyParity() {
  return Object.keys(catalogs.en).every((key) => key in catalogs.ja) && Object.keys(catalogs.ja).every((key) => key in catalogs.en);
}
