/**
 * JSON helpers replicating `serde_json`'s BTreeMap-backed ordering.
 *
 * Rust's `serde_json` default map is a `BTreeMap`, so every JSON document the
 * Rust tool writes — Preferences/Local State files and stdout payloads — has
 * alphabetically sorted object keys, and pretty output uses 2-space indent.
 * JS objects preserve insertion order, so we recursively sort keys before
 * serializing. This keeps rewritten files byte-compatible with the Rust tool.
 */
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | JsonValue[]
  | { [key: string]: JsonValue };

export type JsonObject = { [key: string]: JsonValue };

/** Recursively sort object keys (arrays keep their order). */
export function sortKeys(value: JsonValue): JsonValue {
  if (Array.isArray(value)) {
    return value.map(sortKeys);
  }
  if (value !== null && typeof value === 'object') {
    const out: JsonObject = {};
    for (const key of Object.keys(value).sort()) {
      out[key] = sortKeys(value[key] as JsonValue);
    }
    return out;
  }
  return value;
}

/** Pretty (2-space indent) serialization with sorted keys — `to_string_pretty` parity. */
export function sortedStringify(value: JsonValue): string {
  return JSON.stringify(sortKeys(value), null, 2);
}

/** Compact serialization with sorted keys — `to_string` parity. */
export function sortedStringifyCompact(value: JsonValue): string {
  return JSON.stringify(sortKeys(value));
}