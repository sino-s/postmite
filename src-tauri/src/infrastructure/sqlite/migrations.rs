pub(super) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "create_workspace_tables",
        sql: r#"
CREATE TABLE workspaces (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE TABLE workspace_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    selected_workspace_id TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (selected_workspace_id) REFERENCES workspaces(id)
        ON UPDATE CASCADE
        ON DELETE RESTRICT
);
"#,
    },
    Migration {
        version: 2,
        name: "create_request_tables",
        sql: r#"
CREATE TABLE collections (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE saved_requests (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    collection_id TEXT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    method TEXT NOT NULL CHECK (length(method) > 0),
    url TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (collection_id, workspace_id) REFERENCES collections(id, workspace_id)
);

CREATE TABLE saved_request_query_rows (
    saved_request_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (saved_request_id, row_order),
    FOREIGN KEY (saved_request_id) REFERENCES saved_requests(id) ON DELETE CASCADE
);

CREATE TABLE saved_request_header_rows (
    saved_request_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (saved_request_id, row_order),
    FOREIGN KEY (saved_request_id) REFERENCES saved_requests(id) ON DELETE CASCADE
);

CREATE TABLE request_drafts (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    saved_request_id TEXT,
    name TEXT NOT NULL CHECK (length(name) > 0),
    method TEXT NOT NULL CHECK (length(method) > 0),
    url TEXT NOT NULL,
    is_dirty INTEGER NOT NULL CHECK (is_dirty IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (saved_request_id, workspace_id) REFERENCES saved_requests(id, workspace_id)
);

CREATE TABLE request_draft_query_rows (
    draft_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (draft_id, row_order),
    FOREIGN KEY (draft_id) REFERENCES request_drafts(id) ON DELETE CASCADE
);

CREATE TABLE request_draft_header_rows (
    draft_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (draft_id, row_order),
    FOREIGN KEY (draft_id) REFERENCES request_drafts(id) ON DELETE CASCADE
);

CREATE TABLE request_tabs (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    saved_request_id TEXT,
    draft_id TEXT NOT NULL UNIQUE,
    position INTEGER NOT NULL CHECK (position >= 0),
    title TEXT NOT NULL CHECK (length(title) > 0),
    is_active INTEGER NOT NULL CHECK (is_active IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (saved_request_id, workspace_id) REFERENCES saved_requests(id, workspace_id),
    FOREIGN KEY (draft_id, workspace_id) REFERENCES request_drafts(id, workspace_id)
        ON DELETE CASCADE
);

CREATE UNIQUE INDEX request_tabs_one_saved_request_per_workspace
    ON request_tabs(workspace_id, saved_request_id)
    WHERE saved_request_id IS NOT NULL;
"#,
    },
    Migration {
        version: 3,
        name: "add_raw_request_body",
        sql: r#"
ALTER TABLE saved_requests ADD COLUMN body TEXT NOT NULL DEFAULT '';
ALTER TABLE request_drafts ADD COLUMN body TEXT NOT NULL DEFAULT '';
"#,
    },
    Migration {
        version: 4,
        name: "add_collection_tree_ordering",
        sql: r#"
ALTER TABLE collections ADD COLUMN parent_collection_id TEXT;
ALTER TABLE collections ADD COLUMN position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0);
ALTER TABLE saved_requests ADD COLUMN position INTEGER NOT NULL DEFAULT 0 CHECK (position >= 0);

CREATE INDEX collections_workspace_parent_position
    ON collections(workspace_id, parent_collection_id, position, created_at, id);
CREATE INDEX saved_requests_workspace_collection_position
    ON saved_requests(workspace_id, collection_id, position, created_at, id);
"#,
    },
    Migration {
        version: 5,
        name: "create_environment_variable_tables",
        sql: r#"
CREATE TABLE environments (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    position INTEGER NOT NULL CHECK (position >= 0),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    UNIQUE (workspace_id, name),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE selected_environments (
    workspace_id TEXT PRIMARY KEY,
    environment_id TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE,
    FOREIGN KEY (environment_id, workspace_id) REFERENCES environments(id, workspace_id)
        ON DELETE SET NULL
);

CREATE TABLE collection_variables (
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    plain_value TEXT,
    secret_ref TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (workspace_id, name),
    CHECK ((plain_value IS NOT NULL AND secret_ref IS NULL) OR (plain_value IS NULL AND secret_ref IS NOT NULL)),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE environment_variables (
    environment_id TEXT NOT NULL,
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    plain_value TEXT,
    secret_ref TEXT,
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    PRIMARY KEY (environment_id, name),
    CHECK ((plain_value IS NOT NULL AND secret_ref IS NULL) OR (plain_value IS NULL AND secret_ref IS NOT NULL)),
    FOREIGN KEY (environment_id, workspace_id) REFERENCES environments(id, workspace_id)
        ON DELETE CASCADE
);

CREATE INDEX environments_workspace_position
    ON environments(workspace_id, position, created_at, id);
CREATE INDEX environment_variables_workspace
    ON environment_variables(workspace_id, environment_id, name);
"#,
    },
    Migration {
        version: 6,
        name: "create_execution_history_tables",
        sql: r#"
CREATE TABLE execution_history_settings (
    workspace_id TEXT PRIMARY KEY,
    disabled INTEGER NOT NULL CHECK (disabled IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE execution_records (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    created_at_epoch_seconds INTEGER NOT NULL,
    pinned INTEGER NOT NULL CHECK (pinned IN (0, 1)),
    name TEXT NOT NULL CHECK (length(name) > 0),
    method TEXT NOT NULL CHECK (length(method) > 0),
    url TEXT NOT NULL,
    body TEXT NOT NULL,
    response_status INTEGER,
    response_body_preview TEXT NOT NULL,
    response_body_truncated INTEGER NOT NULL CHECK (response_body_truncated IN (0, 1)),
    response_error TEXT,
    response_duration_ms INTEGER,
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE TABLE execution_record_query_rows (
    execution_record_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (execution_record_id, row_order),
    FOREIGN KEY (execution_record_id) REFERENCES execution_records(id) ON DELETE CASCADE
);

CREATE TABLE execution_record_header_rows (
    execution_record_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (execution_record_id, row_order),
    FOREIGN KEY (execution_record_id) REFERENCES execution_records(id) ON DELETE CASCADE
);

CREATE TABLE execution_record_response_header_rows (
    execution_record_id TEXT NOT NULL,
    row_order INTEGER NOT NULL CHECK (row_order >= 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    name TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (execution_record_id, row_order),
    FOREIGN KEY (execution_record_id) REFERENCES execution_records(id) ON DELETE CASCADE
);

CREATE INDEX execution_records_workspace_created
    ON execution_records(workspace_id, created_at_epoch_seconds DESC, id);
CREATE INDEX execution_records_workspace_unpinned_created
    ON execution_records(workspace_id, pinned, created_at_epoch_seconds, id);
"#,
    },
    Migration {
        version: 7,
        name: "create_workspace_cookie_metadata",
        sql: r#"
CREATE TABLE workspace_cookies (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    name TEXT NOT NULL CHECK (length(name) > 0),
    domain TEXT NOT NULL CHECK (length(domain) > 0),
    path TEXT NOT NULL CHECK (length(path) > 0),
    secure INTEGER NOT NULL CHECK (secure IN (0, 1)),
    http_only INTEGER NOT NULL CHECK (http_only IN (0, 1)),
    same_site TEXT CHECK (same_site IN ('strict', 'lax', 'none')),
    expires_at_epoch_seconds INTEGER,
    session INTEGER NOT NULL CHECK (session IN (0, 1)),
    has_value INTEGER NOT NULL CHECK (has_value IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    UNIQUE (workspace_id, name, domain, path),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX workspace_cookies_workspace_scope
    ON workspace_cookies(workspace_id, domain, path, secure, expires_at_epoch_seconds);
"#,
    },
    Migration {
        version: 8,
        name: "add_workspace_base_directory",
        sql: r#"
ALTER TABLE workspaces ADD COLUMN base_directory TEXT;
"#,
    },
    Migration {
        version: 9,
        name: "add_request_security_policy",
        sql: r#"
ALTER TABLE saved_requests ADD COLUMN auth TEXT NOT NULL DEFAULT '{"type":"NONE"}';
ALTER TABLE saved_requests ADD COLUMN redirect_policy TEXT NOT NULL DEFAULT '{"enabled":true,"maxRedirects":10}';
ALTER TABLE saved_requests ADD COLUMN tls_policy TEXT NOT NULL DEFAULT '{"verify":true,"customCaReference":null,"clientCertificateReference":null,"clientKeyReference":null}';

ALTER TABLE request_drafts ADD COLUMN auth TEXT NOT NULL DEFAULT '{"type":"NONE"}';
ALTER TABLE request_drafts ADD COLUMN redirect_policy TEXT NOT NULL DEFAULT '{"enabled":true,"maxRedirects":10}';
ALTER TABLE request_drafts ADD COLUMN tls_policy TEXT NOT NULL DEFAULT '{"verify":true,"customCaReference":null,"clientCertificateReference":null,"clientKeyReference":null}';

ALTER TABLE execution_records ADD COLUMN auth TEXT NOT NULL DEFAULT '{"type":"NONE"}';
ALTER TABLE execution_records ADD COLUMN redirect_policy TEXT NOT NULL DEFAULT '{"enabled":true,"maxRedirects":10}';
ALTER TABLE execution_records ADD COLUMN tls_policy TEXT NOT NULL DEFAULT '{"verify":true,"customCaReference":null,"clientCertificateReference":null,"clientKeyReference":null}';
"#,
    },
    Migration {
        version: 10,
        name: "add_request_transport_policy",
        sql: r#"
ALTER TABLE saved_requests ADD COLUMN transport_policy TEXT NOT NULL DEFAULT '{"proxy":{"source":"PROCESS_ENVIRONMENT","url":null,"noProxy":[]},"timeouts":{"connectMs":10000,"overallMs":300000,"idleMs":60000}}';
ALTER TABLE request_drafts ADD COLUMN transport_policy TEXT NOT NULL DEFAULT '{"proxy":{"source":"PROCESS_ENVIRONMENT","url":null,"noProxy":[]},"timeouts":{"connectMs":10000,"overallMs":300000,"idleMs":60000}}';
ALTER TABLE execution_records ADD COLUMN transport_policy TEXT NOT NULL DEFAULT '{"proxy":{"source":"PROCESS_ENVIRONMENT","url":null,"noProxy":[]},"timeouts":{"connectMs":10000,"overallMs":300000,"idleMs":60000}}';
"#,
    },
    Migration {
        version: 11,
        name: "add_cookie_secret_references",
        sql: r#"
ALTER TABLE workspace_cookies ADD COLUMN secret_ref TEXT;
"#,
    },
    Migration {
        version: 12,
        name: "create_postman_import_records",
        sql: r#"
CREATE TABLE postman_import_records (
    id TEXT PRIMARY KEY CHECK (length(id) > 0),
    workspace_id TEXT NOT NULL,
    source_id TEXT NOT NULL CHECK (length(source_id) > 0),
    source_name TEXT NOT NULL CHECK (length(source_name) > 0),
    source_hash TEXT NOT NULL CHECK (length(source_hash) > 0),
    collection_json_sha256 TEXT NOT NULL CHECK (length(collection_json_sha256) > 0),
    environment_json_sha256 TEXT,
    warning_count INTEGER NOT NULL CHECK (warning_count >= 0),
    unsupported_count INTEGER NOT NULL CHECK (unsupported_count >= 0),
    warnings_json TEXT NOT NULL,
    unsupported_json TEXT NOT NULL,
    imported_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE (id, workspace_id),
    FOREIGN KEY (workspace_id) REFERENCES workspaces(id) ON DELETE CASCADE
);

CREATE INDEX postman_import_records_workspace_imported
    ON postman_import_records(workspace_id, imported_at DESC, id);
"#,
    },
    Migration {
        version: 13,
        name: "track_postman_import_entities",
        sql: r#"
ALTER TABLE postman_import_records ADD COLUMN collection_ids_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE postman_import_records ADD COLUMN environment_ids_json TEXT NOT NULL DEFAULT '[]';
"#,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Migration {
    pub(super) version: i64,
    pub(super) name: &'static str,
    pub(super) sql: &'static str,
}
