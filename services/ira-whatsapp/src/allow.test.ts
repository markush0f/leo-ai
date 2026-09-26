import assert from "node:assert/strict";
import test from "node:test";
import { isSelfChat, mayReply, parseAllowPhones } from "./allow.ts";

const own = ["34600111222:14@s.whatsapp.net", "999@lid"];

test("self-chat matches phone and lid", () => {
  assert.equal(isSelfChat("34600111222@s.whatsapp.net", own), true);
  assert.equal(isSelfChat("999@lid", own), true);
  assert.equal(isSelfChat("34600999888@s.whatsapp.net", own), false);
});

test("personal inbox stays quiet unless allowlisted", () => {
  const allow = parseAllowPhones("+34 600 999 888, 111");
  assert.deepEqual(allow, ["34600999888", "111"]);
  assert.equal(
    mayReply({
      remoteJid: "34600999888@s.whatsapp.net",
      fromMe: false,
      ownJids: own,
      allowPhones: allow,
    }),
    true,
  );
  assert.equal(
    mayReply({
      remoteJid: "34600888777@s.whatsapp.net",
      fromMe: false,
      ownJids: own,
      allowPhones: allow,
    }),
    false,
  );
  assert.equal(
    mayReply({
      remoteJid: "34600999888@s.whatsapp.net",
      fromMe: true,
      ownJids: own,
      allowPhones: allow,
    }),
    false,
  );
});

test("self-chat accepts fromMe and groups never reply", () => {
  assert.equal(
    mayReply({
      remoteJid: "34600111222@s.whatsapp.net",
      fromMe: true,
      ownJids: own,
      allowPhones: [],
    }),
    true,
  );
  assert.equal(
    mayReply({
      remoteJid: "1203630@g.us",
      fromMe: false,
      ownJids: own,
      allowPhones: ["1203630"],
    }),
    false,
  );
});
