import { test, expect, type Page } from "@playwright/test";
import { parseDatabaseUrl } from "../src/database-url";

const snapshot = {
  providers: [{ id: "p1", name: "Ollama", kind: "ollama", base_url: null, key: "none" }],
  models: [{ id: "m1", provider_id: "p1", name: "qwen3:8b", effort: "low" }], engines: [],
  active_model_id: "m1", active_conversation_id: null, system: "Responde en español.",
  voice_system: "", stt_engine_id: null, tts_engine_id: null, wake_engine_id: null,
  stt_language: "es", thinking: true, tools_enabled: true, tools: ["get_weather", "database_query"],
};

async function mockWorkspace(page: Page, turns: { id: string; role: string; content: string }[] = []) {
  await page.route("**/api/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    let body: unknown = {};
    if (path === "/api/snapshot" || path === "/api/apply") body = snapshot;
    else if (path === "/api/services") body = { ok: true, services: [
      { id: "postgres", name: "Postgres", running: true, healthy: true, detail: "Disponible" },
      { id: "toolbox", name: "Toolbox", running: true, healthy: true, detail: "Disponible" },
    ] };
    else if (path === "/api/chats") body = route.request().method() === "POST" ? { id: "c2", title: null } : [{ id: "c1", title: "Ideas para mi próximo proyecto" }];
    else if (path === "/api/chats/c1") body = turns;
    else if (path === "/api/databases") body = [];
    await route.fulfill({ json: body });
  });
}

test("PostgreSQL URLs preserve encoded credentials, IPv6 and SSL; unsupported values fail safely", () => {
  const parsed = parseDatabaseUrl("postgresql://reader:p%40ss%3Aword@[::1]:6543/my%20db?sslmode=require");
  expect(parsed).toMatchObject({ host: "::1", port: 6543, database: "my db", username: "reader", password: "p@ss:word", ssl_mode: "require" });
  expect(parseDatabaseUrl("postgres://reader@localhost/ira").port).toBe(5432);
  for (const input of ["https://reader@host/ira", "postgres://host/ira", "postgres://reader@host/", "postgres://reader:secret@host:70000/ira", "postgres://reader@host/ira?sslmode=unknown", "postgres://reader@host/ira?sslcert=file", "postgres://reader:%zz@host/ira"]) {
    expect(() => parseDatabaseUrl(input)).toThrow();
    try { parseDatabaseUrl(input); } catch (error) { expect(String(error)).not.toContain("secret"); }
  }
});

test("sidebar resizes, compacts and restores preferences", async ({ page }) => {
  await mockWorkspace(page);
  await page.goto("/");
  const separator = page.getByRole("separator", { name: "Ancho de la barra lateral" });
  await separator.focus();
  await page.keyboard.press("End");
  await expect(separator).toHaveAttribute("aria-valuenow", "360");
  await page.reload();
  await expect(separator).toHaveAttribute("aria-valuenow", "360");
  await page.getByRole("button", { name: "Compactar barra lateral" }).click();
  await expect(page.locator(".rail")).toHaveCSS("width", "76px");
  await page.reload();
  await expect(page.locator(".rail")).toHaveCSS("width", "76px");
  await page.getByRole("button", { name: "Ampliar barra lateral" }).click();
  await expect(separator).toHaveAttribute("aria-valuenow", "360");
  await expect(page.locator(".rail")).toHaveCSS("width", "360px");
  const box = await separator.boundingBox();
  await page.mouse.move(box!.x + 4, box!.y + 120);
  await page.mouse.down();
  await page.mouse.move(box!.x - 96, box!.y + 120);
  await page.mouse.up();
  await expect(separator).toHaveAttribute("aria-valuenow", "260");
});

test("URL import, save/test failure, correction and retry retain one connection", async ({ page }) => {
  await mockWorkspace(page);
  let saved: Record<string, unknown> | undefined;
  let creates = 0;
  let tests = 0;
  await page.route("**/api/databases**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    const method = route.request().method();
    if (path.endsWith("/test")) {
      tests++;
      await route.fulfill({ json: { ok: tests > 1, read_only: true, detail: tests > 1 ? "Acceso verificado." : "No se pudo acceder al servidor. Revisa el host." } });
    } else if (method === "POST" || method === "PUT") {
      if (method === "POST") creates++;
      saved = route.request().postDataJSON();
      await route.fulfill({ json: { ...saved, id: "db1", password_set: true, last_test_ok: null, last_test_error: null, last_tested_at: null } });
    } else await route.fulfill({ json: [] });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Bases de datos", exact: true }).click();
  await page.getByLabel("¿Tienes una URL de conexión?").fill("postgresql://reader:p%40ss@db.local:5433/analytics?sslmode=require");
  await page.getByRole("button", { name: "Completar desde URL" }).click();
  await expect(page.getByLabel("Servidor", { exact: true })).toHaveValue("db.local");
  await expect(page.getByLabel("Contraseña", { exact: true })).toHaveValue("p@ss");
  await expect(page.getByLabel("¿Tienes una URL de conexión?")).toHaveValue("");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Falló la conexión" })).toBeVisible();
  await page.getByLabel("Servidor", { exact: true }).fill("db.correct.local");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Conexión correcta" })).toBeVisible();
  expect(creates).toBe(1);
  expect(tests).toBe(2);
  expect(saved?.host).toBe("db.correct.local");
  expect(saved).not.toHaveProperty("password");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Bases de datos", exact: true })).toBeFocused();
});

test("sheet traps focus and exposes shared input controls", async ({ page }) => {
  await mockWorkspace(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Modelos y configuración", exact: true }).click();
  const close = page.getByRole("dialog").getByRole("button", { name: "Cerrar catálogo", exact: true });
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  expect(await page.evaluate(() => Boolean(document.activeElement?.closest('[role="dialog"]')))).toBe(true);
  await page.getByLabel("Buscar modelo").fill("qwen");
  await expect(page.getByRole("button", { name: "Modelo en uso: qwen3:8b" })).toBeVisible();
});

test("stream completion respects reading position and renders every delta", async ({ page }) => {
  const turns = Array.from({ length: 18 }, (_, index) => ({ id: `t${index}`, role: index % 2 ? "assistant" : "user", content: `Mensaje ${index}. ` + "Texto de prueba para una conversación larga. ".repeat(10) }));
  await mockWorkspace(page, turns);
  let release!: () => void;
  const gate = new Promise<void>((resolve) => { release = resolve; });
  await page.route("**/messages/stream", async (route) => {
    await gate;
    await route.fulfill({ contentType: "application/x-ndjson", body: [...Array.from({ length: 50 }, (_, i) => JSON.stringify({ type: "delta", text: `fragmento-${i} ` })), JSON.stringify({ type: "done" })].join("\n") });
  });
  await page.goto("/");
  await page.getByRole("textbox", { name: "Mensaje para Ira" }).fill("Continúa");
  await page.getByRole("button", { name: "Enviar mensaje" }).click();
  await expect(page.getByText("Razonando tu respuesta")).toBeVisible();
  await page.locator(".log").evaluate((el) => { el.scrollTop = 0; });
  await expect(page.getByRole("button", { name: "Ir al último mensaje" })).toBeVisible();
  release();
  await expect(page.locator(".turn.ira").last()).toContainText("fragmento-49");
  await expect(page.locator(".sr-only[role=status]")).toContainText("Ira: fragmento-0");
  await expect(page.locator(".sr-only[role=status]")).toContainText("fragmento-49");
  expect(await page.locator(".log").evaluate((el) => el.scrollTop)).toBeLessThan(30);
  await page.getByRole("button", { name: "Ir al último mensaje" }).click();
  await expect.poll(() => page.locator(".log").evaluate((el) => el.scrollHeight - el.scrollTop - el.clientHeight)).toBeLessThan(10);
});

test("visual states: desktop dark/light and mobile, no horizontal overflow", async ({ page }) => {
  await mockWorkspace(page);
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/?theme=dark");
  await expect(page.getByRole("textbox", { name: "Mensaje para Ira" })).toBeEnabled();
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(650);
  await page.screenshot({ path: "../.impeccable/review/desktop.png", fullPage: true, animations: "disabled" });
  await page.goto("/?demo");
  await expect(page.locator(".turn.ira")).toBeVisible();
  await page.waitForTimeout(650);
  await page.screenshot({ path: "../.impeccable/review/desktop-chat.png", fullPage: true, animations: "disabled" });
  await page.goto("/?theme=light");
  await page.getByRole("button", { name: "Bases de datos", exact: true }).click();
  await expect(page.getByLabel("Servidor", { exact: true })).toBeVisible();
  await page.waitForTimeout(500);
  await page.screenshot({ path: "../.impeccable/review/desktop-database.png", fullPage: true, animations: "disabled" });
  await page.keyboard.press("Escape");
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/?theme=dark");
  await expect(page.getByRole("textbox", { name: "Mensaje para Ira" })).toBeEnabled();
  await page.waitForTimeout(650);
  await page.screenshot({ path: "../.impeccable/review/mobile.png", fullPage: true, animations: "disabled" });
  await page.getByRole("button", { name: "mostrar barra lateral" }).click();
  await expect(page.getByRole("button", { name: "Cerrar menú", exact: true })).toBeFocused();
  await page.waitForTimeout(300);
  await page.screenshot({ path: "../.impeccable/review/mobile-drawer.png", fullPage: true, animations: "disabled" });
  await page.getByRole("button", { name: "Bases de datos", exact: true }).click();
  await expect(page.getByLabel("Servidor", { exact: true })).toBeVisible();
  await page.waitForTimeout(500);
  await page.screenshot({ path: "../.impeccable/review/mobile-database.png", fullPage: true, animations: "disabled" });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  expect(errors).toEqual([]);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.keyboard.press("Escape");
  await page.goto("/?demo");
  await expect(page.locator(".turn.ira")).toBeVisible();
  await expect(page.locator(".hero-logo")).toHaveCount(0);
  await expect(page.getByText("Conversación de ejemplo · datos simulados")).toBeVisible();
});

test("mobile drawer manages keyboard focus, close and desktop breakpoint", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await mockWorkspace(page);
  await page.goto("/");
  const opener = page.getByRole("button", { name: "mostrar barra lateral" });
  await opener.click();
  const close = page.getByRole("button", { name: "Cerrar menú", exact: true });
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  expect(await page.evaluate(() => Boolean(document.activeElement?.closest(".rail")))).toBe(true);
  await page.keyboard.press("Escape");
  await expect(opener).toBeFocused();
  await opener.click();
  await close.click();
  await expect(opener).toBeFocused();
  await opener.click();
  await page.setViewportSize({ width: 1280, height: 800 });
  await expect(page.locator(".app")).not.toHaveClass(/rail-open/);
  await expect(page.locator(".stage")).not.toHaveAttribute("inert");
  await expect(page.getByRole("button", { name: "Compactar barra lateral" })).toBeFocused();
});
