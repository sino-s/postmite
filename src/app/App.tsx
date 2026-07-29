import { RequestEditor } from "../features/request-editor/RequestEditor";
import { I18nProvider } from "./i18n";

export function App() {
  return (
    <I18nProvider>
      <RequestEditor />
    </I18nProvider>
  );
}
