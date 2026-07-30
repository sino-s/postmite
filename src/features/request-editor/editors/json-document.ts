import { parser } from "@lezer/json";
import type { SyntaxNode, Tree } from "@lezer/common";

export type JsonValidation =
  | { state: "empty" }
  | { state: "valid"; tree: Tree }
  | {
      state: "invalid";
      offset: number;
      line: number;
      column: number;
      diagnosticFrom: number;
      diagnosticTo: number;
    };

function firstError(node: SyntaxNode): SyntaxNode | null {
  if (node.type.isError) {
    return node;
  }

  let child = node.firstChild;
  while (child) {
    const error = firstError(child);
    if (error) {
      return error;
    }
    child = child.nextSibling;
  }
  return null;
}

function lineAndColumn(source: string, offset: number) {
  let line = 1;
  let lineStart = 0;
  for (let index = 0; index < offset; index += 1) {
    if (source.charCodeAt(index) === 10) {
      line += 1;
      lineStart = index + 1;
    }
  }
  return { line, column: offset - lineStart + 1 };
}

export function validateJsonDocument(source: string): JsonValidation {
  if (source.trim().length === 0) {
    return { state: "empty" };
  }

  const tree = parser.parse(source);
  const error = firstError(tree.topNode);
  if (!error) {
    return { state: "valid", tree };
  }

  const offset = Math.min(error.from, source.length);
  const { line, column } = lineAndColumn(source, offset);
  let diagnosticFrom = error.from;
  let diagnosticTo = error.to;
  if (diagnosticFrom === diagnosticTo && source.length > 0) {
    if (diagnosticFrom < source.length) {
      diagnosticTo = diagnosticFrom + 1;
    } else {
      diagnosticFrom -= 1;
    }
  }

  return {
    state: "invalid",
    offset,
    line,
    column,
    diagnosticFrom,
    diagnosticTo,
  };
}

function children(node: SyntaxNode) {
  const result: SyntaxNode[] = [];
  let child = node.firstChild;
  while (child) {
    result.push(child);
    child = child.nextSibling;
  }
  return result;
}

function indent(depth: number) {
  return "  ".repeat(depth);
}

function formatNode(source: string, node: SyntaxNode, depth: number): string {
  switch (node.name) {
    case "JsonText": {
      const value = node.firstChild;
      return value ? formatNode(source, value, depth) : "";
    }
    case "Object": {
      const properties = children(node).filter((child) => child.name === "Property");
      if (properties.length === 0) {
        return "{}";
      }
      return `{\n${properties
        .map((property) => `${indent(depth + 1)}${formatNode(source, property, depth + 1)}`)
        .join(",\n")}\n${indent(depth)}}`;
    }
    case "Property": {
      const parts = children(node);
      const name = parts.find((part) => part.name === "PropertyName");
      const value = parts.find(
        (part) => part.name !== "PropertyName" && part.name !== ":",
      );
      return `${name ? source.slice(name.from, name.to) : ""}: ${value ? formatNode(source, value, depth) : ""}`;
    }
    case "Array": {
      const values = children(node).filter(
        (child) => child.name !== "[" && child.name !== "]" && child.name !== ",",
      );
      if (values.length === 0) {
        return "[]";
      }
      return `[\n${values
        .map((value) => `${indent(depth + 1)}${formatNode(source, value, depth + 1)}`)
        .join(",\n")}\n${indent(depth)}]`;
    }
    default:
      return source.slice(node.from, node.to);
  }
}

export function formatJsonDocument(source: string): string | null {
  const validation = validateJsonDocument(source);
  if (validation.state !== "valid") {
    return null;
  }
  return formatNode(source, validation.tree.topNode, 0);
}
