import assert from "node:assert/strict";
import test from "node:test";
import { route } from "./commands.ts";

test("routes chat and commands", () => {
  assert.equal(route("hola").type, "chat");
  assert.equal(route("/help").type, "help");
  assert.equal(route("/start@bot").type, "help");
  assert.equal(route("/clear ya").type, "clear");
  assert.deepEqual(route("/nope"), { type: "unknown", name: "nope" });
});
