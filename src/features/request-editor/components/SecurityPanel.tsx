import type { RequestContentDto, ResolvedRequestContentDto } from "../../../shared/api/generated/ipc";
import { Input } from "../../../components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "../../../components/ui/select";

type SecurityPanelProps = {
  content: RequestContentDto;
  onChange: (updater: (content: RequestContentDto) => RequestContentDto) => void;
  resolution: ResolvedRequestContentDto | null;
};

export function SecurityPanel({ content, onChange, resolution }: SecurityPanelProps) {
  const authType = content.auth.type;
  return (
    <section
      aria-label="Security policy"
      className="grid shrink-0 gap-3 rounded-md border border-slate-300 bg-white p-3 text-sm"
    >
      <div className="flex items-center justify-between gap-3">
        <h2 className="text-sm font-semibold text-slate-950">Security</h2>
        {!content.tls.verify || resolution?.unsafeTlsVisible ? (
          <span className="rounded-md border border-amber-300 bg-amber-50 px-2 py-1 text-xs font-semibold text-amber-900">
            TLS verification off
          </span>
        ) : null}
      </div>
      <div className="grid gap-2 sm:grid-cols-3">
        <label className="grid gap-1 text-xs font-medium text-slate-700">
          Auth
          <Select
            onValueChange={(type) => {
              onChange((current) => ({
                ...current,
                auth:
                  type === "BASIC"
                    ? { type: "BASIC", username: "", password: "" }
                    : type === "BEARER"
                      ? { type: "BEARER", token: "" }
                      : type === "API_KEY"
                        ? {
                            type: "API_KEY",
                            placement: "HEADER",
                            name: "",
                            value: "",
                          }
                        : type === "CLIENT_CREDENTIALS"
                          ? {
                              type: "CLIENT_CREDENTIALS",
                              tokenEndpoint: "",
                              clientId: "",
                              clientSecret: "",
                              scopes: [],
                            }
                        : { type: "NONE" },
              }));
            }}
            value={authType}
          >
            <SelectTrigger>
              <SelectValue placeholder="Auth" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="NONE">No Auth</SelectItem>
              <SelectItem value="BASIC">Basic</SelectItem>
              <SelectItem value="BEARER">Bearer</SelectItem>
              <SelectItem value="API_KEY">API Key</SelectItem>
              <SelectItem value="CLIENT_CREDENTIALS">Client Credentials</SelectItem>
            </SelectContent>
          </Select>
        </label>
        {content.auth.type === "BASIC" ? (
          <>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Username
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "BASIC"
                      ? {
                          ...current,
                          auth: {
                            ...current.auth,
                            username: event.currentTarget.value,
                          },
                        }
                      : current,
                  )
                }
                value={content.auth.username}
              />
            </label>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Password reference
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "BASIC"
                      ? {
                          ...current,
                          auth: {
                            ...current.auth,
                            password: event.currentTarget.value,
                          },
                        }
                      : current,
                  )
                }
                value={content.auth.password}
              />
            </label>
          </>
        ) : null}
        {content.auth.type === "BEARER" ? (
          <label className="grid gap-1 text-xs font-medium text-slate-700 sm:col-span-2">
            Token reference
            <Input
              onChange={(event) =>
                onChange((current) =>
                  current.auth.type === "BEARER"
                    ? {
                        ...current,
                        auth: { ...current.auth, token: event.currentTarget.value },
                      }
                    : current,
                )
              }
              value={content.auth.token}
            />
          </label>
        ) : null}
        {content.auth.type === "API_KEY" ? (
          <>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Placement
              <Select
                onValueChange={(placement) =>
                  onChange((current) =>
                    current.auth.type === "API_KEY"
                      ? {
                          ...current,
                          auth: {
                            ...current.auth,
                            placement: placement as "HEADER" | "QUERY",
                          },
                        }
                      : current,
                  )
                }
                value={content.auth.placement}
              >
                <SelectTrigger>
                  <SelectValue placeholder="Placement" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="HEADER">Header</SelectItem>
                  <SelectItem value="QUERY">Query</SelectItem>
                </SelectContent>
              </Select>
            </label>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Key name
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "API_KEY"
                      ? {
                          ...current,
                          auth: { ...current.auth, name: event.currentTarget.value },
                        }
                      : current,
                  )
                }
                value={content.auth.name}
              />
            </label>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Value reference
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "API_KEY"
                      ? {
                          ...current,
                          auth: { ...current.auth, value: event.currentTarget.value },
                        }
                      : current,
                  )
                }
                value={content.auth.value}
              />
            </label>
          </>
        ) : null}
        {content.auth.type === "CLIENT_CREDENTIALS" ? (
          <>
            <label className="grid gap-1 text-xs font-medium text-slate-700 sm:col-span-2">
              Token endpoint
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "CLIENT_CREDENTIALS"
                      ? {
                          ...current,
                          auth: { ...current.auth, tokenEndpoint: event.currentTarget.value },
                        }
                      : current,
                  )
                }
                value={content.auth.tokenEndpoint}
              />
            </label>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Client ID
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "CLIENT_CREDENTIALS"
                      ? {
                          ...current,
                          auth: { ...current.auth, clientId: event.currentTarget.value },
                        }
                      : current,
                  )
                }
                value={content.auth.clientId}
              />
            </label>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Client secret reference
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "CLIENT_CREDENTIALS"
                      ? {
                          ...current,
                          auth: { ...current.auth, clientSecret: event.currentTarget.value },
                        }
                      : current,
                  )
                }
                value={content.auth.clientSecret}
              />
            </label>
            <label className="grid gap-1 text-xs font-medium text-slate-700">
              Scopes
              <Input
                onChange={(event) =>
                  onChange((current) =>
                    current.auth.type === "CLIENT_CREDENTIALS"
                      ? {
                          ...current,
                          auth: {
                            ...current.auth,
                            scopes: event.currentTarget.value
                              .split(/\s+/)
                              .filter((scope) => scope.length > 0),
                          },
                        }
                      : current,
                  )
                }
                value={content.auth.scopes.join(" ")}
              />
            </label>
          </>
        ) : null}
      </div>
      <div className="grid gap-2 sm:grid-cols-[auto_140px_1fr_1fr_1fr]">
        <label className="inline-flex items-center gap-2 text-xs font-medium text-slate-700">
          <input
            checked={content.redirect.enabled}
            className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
            onChange={(event) =>
              onChange((current) => ({
                ...current,
                redirect: {
                  ...current.redirect,
                  enabled: event.currentTarget.checked,
                },
              }))
            }
            type="checkbox"
          />
          Redirects
        </label>
        <label className="grid gap-1 text-xs font-medium text-slate-700">
          Max
          <Input
            max={10}
            min={0}
            onChange={(event) =>
              onChange((current) => ({
                ...current,
                redirect: {
                  ...current.redirect,
                  maxRedirects: Number(event.currentTarget.value),
                },
              }))
            }
            type="number"
            value={content.redirect.maxRedirects}
          />
        </label>
        <label className="inline-flex items-center gap-2 text-xs font-medium text-slate-700">
          <input
            checked={content.tls.verify}
            className="h-4 w-4 rounded border-slate-300 text-slate-900 focus:ring-sky-500"
            onChange={(event) =>
              onChange((current) => ({
                ...current,
                tls: { ...current.tls, verify: event.currentTarget.checked },
              }))
            }
            type="checkbox"
          />
          Verify TLS
        </label>
        <TlsReferenceInput
          label="Custom CA"
          onChange={(value) =>
            onChange((current) => ({
              ...current,
              tls: { ...current.tls, customCaReference: value || null },
            }))
          }
          value={content.tls.customCaReference}
        />
        <TlsReferenceInput
          label="Client cert"
          onChange={(value) =>
            onChange((current) => ({
              ...current,
              tls: { ...current.tls, clientCertificateReference: value || null },
            }))
          }
          value={content.tls.clientCertificateReference}
        />
        <TlsReferenceInput
          label="Client key"
          onChange={(value) =>
            onChange((current) => ({
              ...current,
              tls: { ...current.tls, clientKeyReference: value || null },
            }))
          }
          value={content.tls.clientKeyReference}
        />
      </div>
    </section>
  );
}

function TlsReferenceInput({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string | null;
}) {
  return (
    <label className="grid gap-1 text-xs font-medium text-slate-700">
      {label}
        <Input
        className="min-w-0"
        onChange={(event) => onChange(event.currentTarget.value)}
        value={value ?? ""}
      />
    </label>
  );
}
