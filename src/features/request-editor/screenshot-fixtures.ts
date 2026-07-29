import type { ResponseExecutionState } from "../../shared/api/execution";
import type {
  ExecutionHistorySnapshotDto,
  RequestContentDto,
  RequestWorkspaceSnapshotDto,
  ResolvedRequestContentDto,
  WorkspaceCookieDto,
} from "../../shared/api/generated/ipc";

const workspaceId = "screenshot-workspace";
const mainCollectionId = "collection-public-api";
const draftId = "draft-public-api";

const primaryRequest: RequestContentDto = {
  name: "Catalog search",
  method: "POST",
  url: "https://api.example.test/catalog/search?locale=en-US&page=1",
  query: [
    { enabled: true, order: 0, name: "locale", value: "{{locale}}" },
    { enabled: true, order: 1, name: "page", value: "1" },
    { enabled: false, order: 2, name: "debug", value: "false" },
  ],
  headers: [
    { enabled: true, order: 0, name: "Accept", value: "application/json" },
    { enabled: true, order: 1, name: "X-Client", value: "postmite-screenshot" },
  ],
  body: {
    type: "RAW",
    content: JSON.stringify(
      {
        filters: ["featured", "available"],
        pageSize: 25,
        includeFacets: true,
      },
      null,
      2,
    ),
  },
  auth: { type: "NONE" },
  redirect: { enabled: true, maxRedirects: 5 },
  tls: {
    verify: true,
    customCaReference: null,
    clientCertificateReference: null,
    clientKeyReference: null,
  },
  transport: {
    proxy: { source: "PROCESS_ENVIRONMENT", url: null, noProxy: [] },
    timeouts: {
      connectMs: 5_000n,
      overallMs: 30_000n,
      idleMs: 10_000n,
    },
  },
};

export const screenshotSnapshot: RequestWorkspaceSnapshotDto = {
  workspaceId,
  collectionFolders: [
    {
      id: mainCollectionId,
      workspaceId,
      parentCollectionId: null,
      name: "Public examples",
      position: 0,
    },
    {
      id: "collection-reports",
      workspaceId,
      parentCollectionId: mainCollectionId,
      name: "Reports",
      position: 0,
    },
  ],
  environments: [
    {
      id: "environment-review",
      workspaceId,
      name: "Review fixture",
      position: 0,
      isSelected: true,
    },
  ],
  collectionVariables: [],
  environmentVariables: [],
  savedRequests: [
    {
      id: "request-search",
      workspaceId,
      collectionId: mainCollectionId,
      position: 0,
      content: primaryRequest,
    },
    {
      id: "request-health",
      workspaceId,
      collectionId: mainCollectionId,
      position: 1,
      content: {
        ...primaryRequest,
        name: "Service health",
        method: "GET",
        url: "https://api.example.test/health",
        body: { type: "NONE" },
      },
    },
  ],
  drafts: [
    {
      id: draftId,
      workspaceId,
      savedRequestId: "request-search",
      content: primaryRequest,
      isDirty: false,
    },
  ],
  tabs: [
    {
      id: "tab-search",
      workspaceId,
      savedRequestId: "request-search",
      draftId,
      position: 0,
      title: "Catalog search",
      isActive: true,
    },
    {
      id: "tab-health",
      workspaceId,
      savedRequestId: "request-health",
      draftId,
      position: 1,
      title: "Service health",
      isActive: false,
    },
  ],
};

export const screenshotResolution: ResolvedRequestContentDto = {
  url: {
    value: "https://api.example.test/catalog/search?locale=en-US&page=1",
    containsSecret: false,
  },
  body: { value: primaryRequest.body.type === "RAW" ? primaryRequest.body.content : "", containsSecret: false },
  query: [
    {
      enabled: true,
      order: 0,
      name: { value: "locale", containsSecret: false },
      value: { value: "en-US", containsSecret: false },
    },
    {
      enabled: true,
      order: 1,
      name: { value: "page", containsSecret: false },
      value: { value: "1", containsSecret: false },
    },
  ],
  headers: [
    {
      enabled: true,
      order: 0,
      name: { value: "Accept", containsSecret: false },
      value: { value: "application/json", containsSecret: false },
    },
    {
      enabled: true,
      order: 1,
      name: { value: "X-Client", containsSecret: false },
      value: { value: "postmite-screenshot", containsSecret: false },
    },
  ],
  unsafeTlsVisible: false,
  references: [
    {
      name: "locale",
      source: "ENVIRONMENT",
      value: { value: "en-US", containsSecret: false },
    },
  ],
  errors: [],
};

export const screenshotExecution: ResponseExecutionState = {
  draftId,
  executionId: "execution-screenshot",
  phase: "completed",
  startedAtMs: 1_700_000_000_000,
  completedAtMs: 1_700_000_000_184,
  lastSequence: 4n,
  method: "POST",
  url: "https://api.example.test/catalog/search?locale=en-US&page=1",
  tlsVerification: true,
  proxy: {
    source: "PROCESS_ENVIRONMENT",
    selectedProxy: null,
    bypassReason: "no proxy selected for screenshot fixture",
  },
  timeouts: { connectMs: 5_000n, overallMs: 30_000n, idleMs: 10_000n },
  timing: {
    queuedMs: 2n,
    dnsMs: 4n,
    connectMs: 11n,
    tlsMs: 18n,
    firstByteMs: 91n,
    downloadMs: 73n,
    totalMs: 184n,
  },
  redirects: [],
  status: 200,
  protocol: "HTTP/2",
  remoteAddr: "203.0.113.10:443",
  headers: [
    { name: "content-type", value: "application/json" },
    { name: "cache-control", value: "no-store" },
  ],
  bodyPreview: JSON.stringify(
    {
      results: [
        { id: "item-100", name: "Fixture notebook", available: true },
        { id: "item-101", name: "Review desk lamp", available: true },
      ],
      nextPage: 2,
    },
    null,
    2,
  ),
  bodyTruncated: false,
  decodedBytes: 196n,
  wireBytes: 244n,
  responseFile: null,
  error: null,
  uploadProgress: { sentBytes: 87n, totalBytes: 87n },
  downloadProgress: { receivedBytes: 244n, totalBytes: 244n },
};

export const screenshotHistory: ExecutionHistorySnapshotDto = {
  workspaceId,
  disabled: false,
  warning: "Screenshot fixture uses public example data only.",
  records: [
    {
      id: "history-1",
      workspaceId,
      createdAtEpochSeconds: 1_700_000_001n,
      request: primaryRequest,
      response: {
        status: 200,
        headers: [{ enabled: true, order: 0, name: "content-type", value: "application/json" }],
        bodyPreview: "{\"ok\":true}",
        bodyTruncated: false,
        error: null,
        durationMs: 184n,
      },
      pinned: true,
    },
    {
      id: "history-2",
      workspaceId,
      createdAtEpochSeconds: 1_699_999_700n,
      request: {
        ...primaryRequest,
        name: "Service health",
        method: "GET",
        url: "https://api.example.test/health",
        body: { type: "NONE" },
      },
      response: {
        status: 204,
        headers: [],
        bodyPreview: "",
        bodyTruncated: false,
        error: null,
        durationMs: 42n,
      },
      pinned: false,
    },
  ],
};

export const screenshotCookies: WorkspaceCookieDto[] = [];
