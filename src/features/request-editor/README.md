# Request Editor Frontend Map

## Before

- `RequestEditor.tsx`: page coordination, data mutations, media queries, outer resizable layout, request/response panel layout.
- `components/`: request controls, domain panels, shared buttons, and panel tests mixed together.
- Root model files: ordered field helpers, request editor helpers, response viewer helpers, worker client, worker code, and screenshot fixtures.
- `ScreenshotApp.tsx`: duplicated the live request workspace layout for Playwright evidence.

## After

- `RequestEditor.tsx`: stable public entry point and live page coordinator.
- `controls/`: reusable request editor controls such as tabs, request line, icon buttons, and ordered field tables.
- `editors/`: body text editor implementations.
- `fixtures/`: screenshot-only DTO fixtures with no Secret values.
- `hooks/`: browser media query hooks shared by live and screenshot layouts.
- `layout/`: resizable workspace composition shared by `RequestEditor` and `ScreenshotApp`.
- `models/`: pure request, ordered-field, and response viewer model helpers with colocated tests.
- `panels/`: domain panels for collections, request configuration, response, history, cookies, security, resolution, and diagnostics.
- `workers/`: response viewer worker entry points and worker-specific tests.
