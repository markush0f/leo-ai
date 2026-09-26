import assert from "node:assert/strict";
import test from "node:test";
import { splitWhatsApp } from "./split.ts";

test("short text stays one chunk", () => {
  assert.deepEqual(splitWhatsApp("hola"), ["hola"]);
});

test("splits on newline before the limit", () => {
  const text = `${"a".repeat(10)}\n${"b".repeat(10)}`;
  assert.deepEqual(splitWhatsApp(text, 12), ["a".repeat(10), "b".repeat(10)]);
});
