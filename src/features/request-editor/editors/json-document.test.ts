import { describe, expect, it } from "vitest";

import { formatJsonDocument, validateJsonDocument } from "./json-document";

describe("JSON request body document tools", () => {
  it("formats nested objects and arrays with deterministic two-space indentation", () => {
    expect(formatJsonDocument('{"outer":{"items":[1,{"ok":true}]},"empty":[]}')).toBe(
      `{
  "outer": {
    "items": [
      1,
      {
        "ok": true
      }
    ]
  },
  "empty": []
}`,
    );
  });

  it("preserves numeric lexemes, duplicate members, order, and string escapes", () => {
    const source =
      '{"unsafe":9007199254740993,"long":123456789012345678901234567890,"decimal":1.2300,"exponent":6.02e+23,"same":1,"same":2,"order":["first","second"],"escaped":"\\\\u0041","unicode":"雪","slashes":"a\\\\/b"}';
    const formatted = formatJsonDocument(source);

    expect(formatted).not.toBeNull();
    for (const token of [
      "9007199254740993",
      "123456789012345678901234567890",
      "1.2300",
      "6.02e+23",
      '"escaped": "\\\\u0041"',
      '"unicode": "雪"',
      '"slashes": "a\\\\/b"',
    ]) {
      expect(formatted).toContain(token);
    }
    expect(formatted?.match(/"same"/g)).toHaveLength(2);
    expect(formatted?.indexOf('"unsafe"')).toBeLessThan(formatted?.indexOf('"long"') ?? 0);
    expect(formatted?.indexOf('"first"')).toBeLessThan(formatted?.indexOf('"second"') ?? 0);
  });

  it.each([
    '{"missing":',
    '{"trailing":true,}',
    "[1,]",
    '"unterminated',
    "01",
    "1.",
    "true false",
    "// comment\n{}",
  ])("rejects malformed JSON without producing replacement text: %s", (source) => {
    expect(validateJsonDocument(source).state).toBe("invalid");
    expect(formatJsonDocument(source)).toBeNull();
  });

  it("reports the first malformed location by line and column", () => {
    const validation = validateJsonDocument('{\n  "ok": true,\n  "broken":\n}');
    expect(validation).toMatchObject({
      state: "invalid",
      line: 4,
      column: 1,
    });
  });

  it.each(["", " ", "\n\t"])("treats empty JSON as neutral: %j", (source) => {
    expect(validateJsonDocument(source)).toEqual({ state: "empty" });
    expect(formatJsonDocument(source)).toBeNull();
  });
});
