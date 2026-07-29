import { RequestEditor } from "../features/request-editor/RequestEditor";
import { I18nProvider } from "./i18n";
import { PreferencesProvider } from "./preferences";

export function App() {
  return (
    <I18nProvider>
      <PreferencesProvider>
        <RequestEditor />
      </PreferencesProvider>
    </I18nProvider>
  );
}
