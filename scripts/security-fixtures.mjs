export const fixtureSecrets = [
  {
    label: "protected variable",
    value: fixtureSecret("PROTECTED_VARIABLE"),
  },
  {
    label: "cookie value",
    value: fixtureSecret("COOKIE_VALUE"),
  },
  {
    label: "session cookie",
    value: fixtureSecret("SESSION_COOKIE_VALUE"),
  },
  {
    label: "persistent cookie",
    value: fixtureSecret("PERSISTENT_COOKIE_VALUE"),
  },
  {
    label: "auth credential",
    value: fixtureSecret("AUTH_CREDENTIAL"),
  },
  {
    label: "basic password",
    value: fixtureSecret("BASIC_PASSWORD"),
  },
  {
    label: "auth header",
    value: fixtureSecret("AUTH_HEADER"),
  },
  {
    label: "cookie header",
    value: fixtureSecret("COOKIE_HEADER"),
  },
  {
    label: "response cookie",
    value: fixtureSecret("RESPONSE_COOKIE"),
  },
  {
    label: "proxy credential",
    value: fixtureSecret("PROXY_CREDENTIAL"),
  },
  {
    label: "private key passphrase",
    value: fixtureSecret("PRIVATE_KEY_PASSPHRASE"),
  },
  {
    label: "oauth code",
    value: fixtureSecret("OAUTH_CODE"),
  },
  {
    label: "oauth access token",
    value: fixtureSecret("OAUTH_ACCESS_TOKEN"),
  },
  {
    label: "oauth refresh token",
    value: fixtureSecret("OAUTH_REFRESH_TOKEN"),
  },
  {
    label: "oauth client secret",
    value: fixtureSecret("OAUTH_CLIENT_SECRET"),
  },
  {
    label: "oauth callback state",
    value: fixtureSecret("OAUTH_CALLBACK_STATE"),
  },
];

function fixtureSecret(name) {
  return ["POSTMITE", "SECRET", name, "29"].join("_");
}
