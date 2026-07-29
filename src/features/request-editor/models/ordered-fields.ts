import type { OrderedFieldDto } from "../../../shared/api/generated/ipc";

export function sortOrderedFields(fields: OrderedFieldDto[]) {
  return [...fields].sort((left, right) => left.order - right.order);
}

export function createEmptyField(order: number): OrderedFieldDto {
  return {
    enabled: true,
    order,
    name: "",
    value: "",
  };
}

export function normalizeFieldOrders(
  fields: OrderedFieldDto[],
): OrderedFieldDto[] {
  return fields.map((field, index) => ({
    ...field,
    order: index,
  }));
}

export function queryFromUrl(url: string): OrderedFieldDto[] {
  const questionIndex = url.indexOf("?");
  if (questionIndex < 0) {
    return [];
  }

  const hashIndex = url.indexOf("#", questionIndex);
  const queryText =
    hashIndex >= 0
      ? url.slice(questionIndex + 1, hashIndex)
      : url.slice(questionIndex + 1);

  if (queryText.length === 0) {
    return [createEmptyField(0)];
  }

  return queryText.split("&").map((pair, order) => {
    const equalsIndex = pair.indexOf("=");
    const rawName = equalsIndex >= 0 ? pair.slice(0, equalsIndex) : pair;
    const rawValue = equalsIndex >= 0 ? pair.slice(equalsIndex + 1) : "";

    return {
      enabled: true,
      order,
      name: decodeQueryPart(rawName),
      value: decodeQueryPart(rawValue),
    };
  });
}

export function applyQueryToUrl(url: string, fields: OrderedFieldDto[]) {
  const hashIndex = url.indexOf("#");
  const beforeHash = hashIndex >= 0 ? url.slice(0, hashIndex) : url;
  const hash = hashIndex >= 0 ? url.slice(hashIndex) : "";
  const base = beforeHash.split("?")[0] ?? "";
  const query = sortOrderedFields(fields)
    .filter((field) => field.enabled)
    .map(
      (field) =>
        `${encodeQueryPart(field.name)}=${encodeQueryPart(field.value)}`,
    )
    .join("&");

  return `${base}${query.length > 0 ? `?${query}` : ""}${hash}`;
}

function decodeQueryPart(value: string) {
  try {
    return decodeURIComponent(value.replace(/\+/g, " "));
  } catch {
    return value;
  }
}

function encodeQueryPart(value: string) {
  return encodeURIComponent(value).replace(/%20/g, "+");
}
