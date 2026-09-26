import assert from "node:assert/strict";
import test from "node:test";
import { messageText } from "./text.ts";

test("reads plain and extended text only", () => {
  assert.equal(messageText({ conversation: " hola " }), "hola");
  assert.equal(messageText({ extendedTextMessage: { text: "hey" } }), "hey");
  assert.equal(messageText({}), null);
});
