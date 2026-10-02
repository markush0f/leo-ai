import { test, expect, type Page } from "@playwright/test";
import { parseDatabaseUrl } from "../src/database-url";

const snapshot = {
  providers: [{ id: "p1", name: "Ollama", kind: "ollama", base_url: null, key: "none" }],
  models: [{ id: "m1", provider_id: "p1", name: "qwen3:8b", display_name: "qwen3:8b", effort: "low", effort_options: [], reasoning: false }], engines: [],
  active_model_id: "m1", active_conversation_id: null, system: "Responde en español.",
  voice_system: "", stt_engine_id: null, tts_engine_id: null, wake_engine_id: null,
  stt_language: "es", thinking: true, tools_enabled: true, tools_mutate: false, tools: ["get_weather", "database_query"],
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
    else if (path === "/api/mcp") body = [];
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

test("browser login gates workspace, does not persist password, and logs out", async ({ page }) => {
  await mockWorkspace(page);
  let signedIn = false;
  await page.route("**/api/auth/**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === "/api/auth/status") {
      await route.fulfill({ status: signedIn ? 200 : 401, json: signedIn ? { authenticated: true } : { error: "no autorizado" } });
    } else if (path === "/api/auth/logout") {
      signedIn = false;
      await route.fulfill({ json: { ok: true } });
    } else if (path === "/api/auth/password") {
      await route.fulfill({ json: { password: null } });
    } else {
      const valid = route.request().postDataJSON().password === "secret-password";
      signedIn = valid;
      await route.fulfill({ status: valid ? 200 : 401, json: valid ? { ok: true } : { error: "no autorizado" } });
    }
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Entra en Ira" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Nueva conversación" })).toHaveCount(0);
  await page.getByLabel("Contraseña", { exact: true }).fill("wrong");
  await page.getByRole("button", { name: "Entrar" }).click();
  await expect(page.getByText("Contraseña incorrecta.")).toBeVisible();
  await page.getByLabel("Contraseña", { exact: true }).fill("secret-password");
  await page.getByRole("button", { name: "Entrar" }).click();
  await expect(page.getByRole("button", { name: "Nueva conversación" })).toBeVisible();
  expect(await page.evaluate(() => JSON.stringify(localStorage) + JSON.stringify(sessionStorage))).not.toContain("secret-password");
  await page.getByRole("button", { name: "Cerrar sesión" }).click();
  await expect(page.getByLabel("Contraseña", { exact: true })).toBeVisible();
});

test("direct PostgreSQL URL connects without opening the manual form", async ({ page }) => {
  await mockWorkspace(page);
  let saved: Record<string, unknown> | undefined;
  await page.route("**/api/databases**", async (route) => {
    if (route.request().url().endsWith("/test")) return route.fulfill({ json: { ok: true, read_only: true, detail: "Acceso verificado." } });
    if (route.request().method() === "POST") {
      saved = route.request().postDataJSON();
      return route.fulfill({ json: { ...saved, id: "db-url", password_set: true, last_test_ok: null, last_test_error: null, last_tested_at: null } });
    }
    return route.fulfill({ json: [] });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  await page.getByRole("button", { name: "Añadir base de datos" }).click();
  await expect(page.getByLabel("Servidor", { exact: true })).toHaveCount(0);
  await page.getByLabel("URL de conexión PostgreSQL").fill("postgresql://reader:secret@db.local/analytics?sslmode=require");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(page.getByRole("region", { name: "Bases de datos", exact: true })).toContainText("Conexión correcta.");
  expect(saved).toMatchObject({ host: "db.local", database: "analytics", username: "reader", password: "secret", ssl_mode: "require" });
  await expect(page.getByLabel("URL de conexión PostgreSQL")).toHaveCount(0);
  expect(await page.evaluate(() => JSON.stringify(localStorage) + JSON.stringify(sessionStorage))).not.toContain("secret");
});

test("MCP uses key-value authentication and retains a saved connection after a failed check", async ({ page }) => {
  await mockWorkspace(page);
  let saved: Record<string, unknown> | undefined;
  let creates = 0;
  let checks = 0;
  await page.route("**/api/mcp**", async (route) => {
    if (route.request().url().endsWith("/test")) {
      checks++;
      return route.fulfill(checks === 1 ? { status: 502, json: { error: "Servidor no disponible" } } : { json: { tools: ["list_tasks"] } });
    }
    if (route.request().method() === "POST") {
      creates++; saved = { ...route.request().postDataJSON(), id: "tasks", editable: true };
      return route.fulfill({ json: saved });
    }
    return route.fulfill({ json: [] });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  await page.getByRole("button", { name: "Añadir servidor MCP" }).click();
  await page.getByLabel("Nombre del servidor MCP").fill("Mis tareas");
  await page.getByLabel("URL del servidor MCP").fill("https://example.com/mcp");
  await page.getByText("Autenticación y cabeceras", { exact: true }).click();
  await page.getByRole("button", { name: "Añadir cabecera" }).click();
  await page.getByLabel("Cabeceras: nombre 1").fill("Authorization");
  await page.getByLabel("Cabeceras: valor 1").fill("Bearer test-secret");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "Servidor no disponible" })).toBeVisible();
  await expect(page.getByLabel("URL del servidor MCP")).toHaveValue("https://example.com/mcp");
  await expect(page.getByLabel("Dónde está el servidor")).toBeFocused();
  expect(saved).toMatchObject({ headers: { Authorization: "Bearer test-secret" } });
  await page.getByRole("button", { name: "Reintentar", exact: true }).click();
  await expect(page.getByText("Última prueba correcta", { exact: true })).toBeVisible();
  await expect(page.locator('.sr-only[role="status"]').filter({ hasText: "Mis tareas" })).toContainText("1 herramientas descubiertas");
  expect(creates).toBe(1);
  expect(checks).toBe(2);
});

test("replacing an imported PostgreSQL URL saves the new destination and editor restores focus", async ({ page }) => {
  await mockWorkspace(page);
  let saved: Record<string, unknown> | undefined;
  await page.route("**/api/databases**", async (route) => {
    if (route.request().url().endsWith("/test")) return route.fulfill({ json: { ok: true, read_only: true, detail: "Acceso verificado." } });
    if (route.request().method() === "POST") {
      saved = route.request().postDataJSON();
      return route.fulfill({ json: { ...saved, id: "db-url", password_set: true, last_test_ok: null, last_test_error: null, last_tested_at: null } });
    }
    return route.fulfill({ json: [] });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  const add = page.getByRole("button", { name: "Añadir base de datos" });
  await add.click();
  await page.getByLabel("URL de conexión PostgreSQL").fill("postgresql://reader:old-secret@old.local/old-db");
  await page.getByRole("button", { name: "Revisar datos de URL" }).click();
  await page.getByLabel("URL de conexión PostgreSQL").fill("postgresql://reader:new-secret@new.local/new-db");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(add).toBeFocused();
  expect(saved).toMatchObject({ host: "new.local", database: "new-db", password: "new-secret" });
  const configure = page.getByRole("region", { name: "Bases de datos", exact: true }).getByRole("button", { name: "Configurar", exact: true });
  await configure.click();
  await expect(page.getByLabel("Nombre de la conexión")).toBeFocused();
  await page.getByRole("button", { name: "Cancelar", exact: true }).click();
  await expect(configure).toBeFocused();
});

test("provider credentials save explicitly, failed saves retain drafts, and search activates models", async ({ page }) => {
  await mockWorkspace(page);
  let state = { ...snapshot, providers: [...snapshot.providers, { id: "p2", name: "OpenAI", kind: "gpt", base_url: null, key: "falta" }], models: [...snapshot.models, { ...snapshot.models[0], id: "m2", provider_id: "p2", name: "test-model", display_name: "Modelo de prueba" }] };
  const ops: string[] = [];
  let fail = true;
  await page.route("**/api/snapshot", (route) => route.fulfill({ json: state }));
  await page.route("**/api/apply", async (route) => {
    const op = route.request().postDataJSON(); ops.push(op.op);
    if (op.op === "set_api_key" && fail) return route.fulfill({ status: 500, json: { error: "No se pudo guardar" } });
    if (op.op === "set_api_key") state = { ...state, providers: state.providers.map((p) => p.id === op.id ? { ...p, key: "db" } : p) };
    if (op.op === "activate_model") state = { ...state, active_model_id: op.id };
    return route.fulfill({ json: state });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Elegir modelo", exact: true }).click();
  await page.getByLabel("Buscar modelo").fill("OpenAI");
  await expect(page.getByRole("button", { name: "Usar modelo Modelo de prueba" })).toBeDisabled();
  await page.getByRole("button", { name: "Configurar proveedores" }).click();
  await page.getByRole("button", { name: "Conectar", exact: true }).click();
  await page.getByLabel("Clave API", { exact: true }).fill("test-key");
  await page.getByLabel("Nombre del proveedor", { exact: true }).focus();
  expect(ops).toEqual([]);
  await page.getByRole("button", { name: "Guardar cambios", exact: true }).click();
  await expect(page.getByRole("alert").filter({ hasText: "No se pudo guardar" })).toBeVisible();
  await expect(page.getByLabel("Clave API", { exact: true })).toHaveValue("test-key");
  fail = false;
  await page.getByRole("button", { name: "Guardar cambios", exact: true }).click();
  await expect(page.getByLabel("Clave API", { exact: true })).toHaveValue("");
  await page.getByRole("button", { name: "Cerrar ajustes" }).click();
  await page.getByRole("button", { name: "Elegir modelo", exact: true }).click();
  await page.getByLabel("Buscar modelo").fill("OpenAI");
  await page.getByRole("button", { name: "Usar modelo Modelo de prueba" }).click();
  await expect(page.getByRole("dialog", { name: "Elegir modelo" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Elegir modelo", exact: true })).toContainText("Modelo de prueba");
  await expect(page.getByRole("button", { name: "Elegir modelo", exact: true })).toBeFocused();
});

test("MCP activity followed by deltas renders one answer with an expandable tool trace", async ({ page }) => {
  await mockWorkspace(page);
  await page.route("**/messages/stream", (route) => route.fulfill({ contentType: "application/x-ndjson", body: [
    { type: "mcp_used", server_id: "tasks", tool_name: "list_tasks" }, { type: "delta", text: "Estas son " }, { type: "delta", text: "tus tareas." }, { type: "done" },
  ].map((event) => JSON.stringify(event)).join("\n") }));
  await page.goto("/");
  await page.getByRole("textbox", { name: "Mensaje para Ira" }).fill("Mis tareas");
  await page.getByRole("button", { name: "Enviar mensaje" }).click();
  await expect(page.locator(".turn.ira")).toHaveCount(1);
  await expect(page.locator(".turn.ira")).toContainText("Estas son tus tareas.");
  await page.getByText("1 herramienta utilizada", { exact: true }).click();
  await expect(page.locator(".tool-trace")).toContainText("list_tasks");
});

test("redesigned connections, settings and model picker at desktop and mobile", async ({ page }) => {
  await mockWorkspace(page);
  await page.route("**/api/mcp", (route) => route.fulfill({ json: [{ id: "tasks", name: "Mis tareas", transport: "streamable_http", url: "https://example.com/mcp", command: null, args: [], env: {}, headers: {}, enabled: true, editable: true }] }));
  await page.route("**/api/databases", (route) => route.fulfill({ json: [{ id: "analytics", name: "Analítica", host: "db.local", port: 5432, database: "analytics", username: "reader", ssl_mode: "require", enabled: true, password_set: true, last_test_ok: false, last_test_error: "No se pudo acceder al servidor. Revisa el host.", last_tested_at: null }] }));
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  for (const [width, name] of [[1440, "desktop"], [390, "mobile"]] as const) {
    await page.setViewportSize({ width, height: width === 1440 ? 1000 : 844 });
    await page.goto("/?theme=dark");
    if (width === 390) await page.getByRole("button", { name: "mostrar barra lateral" }).click();
    await page.getByRole("button", { name: "Conexiones", exact: true }).click();
    await expect(page.getByText("Analítica", { exact: true })).toBeVisible();
    await expect(page.getByText("Mis tareas", { exact: true })).toBeVisible();
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({ path: `../.impeccable/review/${name}-connections.png`, animations: "disabled" });
    await page.getByRole("button", { name: "Cerrar conexiones" }).click();
    if (width === 390) await page.getByRole("button", { name: "mostrar barra lateral" }).click();
    await page.getByRole("button", { name: "Ajustes", exact: true }).click();
    await page.screenshot({ path: `../.impeccable/review/${name}-settings.png`, animations: "disabled" });
    await page.getByRole("button", { name: "Cerrar ajustes" }).click();
    await page.getByRole("button", { name: "Elegir modelo", exact: true }).click();
    await expect(page.getByLabel("Buscar modelo")).toBeFocused();
    const triggerBox = await page.getByRole("button", { name: "Elegir modelo", exact: true }).boundingBox();
    const dialogBox = await page.getByRole("dialog", { name: "Elegir modelo" }).boundingBox();
    expect(Math.abs(triggerBox!.y - (dialogBox!.y + dialogBox!.height))).toBeLessThan(20);
    await page.screenshot({ path: `../.impeccable/review/${name}-model-picker.png`, animations: "disabled" });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
  expect(errors).toEqual([]);
});

for (const viewport of [{ width: 1440, height: 700 }, { width: 390, height: 568 }]) {
  test(`long sections scroll without moving headers or navigation at ${viewport.width}px`, async ({ page }, testInfo) => {
    await page.setViewportSize(viewport);
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockWorkspace(page);
    await page.route("**/api/snapshot", (route) => route.fulfill({ json: {
      ...snapshot,
      providers: Array.from({ length: 24 }, (_, index) => ({ ...snapshot.providers[0], id: `p${index}`, name: `Proveedor ${index}` })),
    } }));
    await page.route("**/api/chats", (route) => route.fulfill({ json: Array.from({ length: 30 }, (_, index) => ({ id: index ? `history${index}` : "c1", title: `Conversación ${index}` })) }));
    await page.route("**/api/mcp", (route) => route.fulfill({ json: Array.from({ length: 24 }, (_, index) => ({
      id: `server${index}`, name: `Servidor ${index}`, transport: "streamable_http", url: "https://example.com/mcp", command: null, args: [], env: {}, headers: {}, enabled: true, editable: true,
    })) }));
    await page.goto("/?theme=dark");
    await page.evaluate(() => document.fonts.ready);
    const mobile = viewport.width < 861;
    if (mobile) await page.getByRole("button", { name: "mostrar barra lateral" }).click();
    const history = page.getByRole("navigation", { name: "conversaciones", exact: true });
    const historyHeading = history.getByText("Conversaciones", { exact: true });
    const headingBox = await historyHeading.boundingBox();
    const historyList = history.locator(".overflow-y-auto");
    await historyList.hover();
    await page.mouse.wheel(0, 900);
    await expect.poll(() => historyList.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
    expect((await historyHeading.boundingBox())!.y).toBe(headingBox!.y);
    await expect(page.getByRole("navigation", { name: "Secciones de Ira" })).toHaveCSS("overflow-y", "visible");
    await page.getByRole("button", { name: "Ajustes", exact: true }).click();

    for (const [title, panelClass, navName] of [["Ajustes", ".settings-content", "Secciones de ajustes"], ["Conexiones", ".connections-content", "Filtrar conexiones"]] as const) {
      const workspace = page.getByRole("region", { name: title, exact: true });
      const header = workspace.locator(".workspace-header");
      const navigation = workspace.getByRole("navigation", { name: navName });
      const panel = workspace.locator(panelClass);
      const headerBox = await header.boundingBox();
      const navBox = await navigation.boundingBox();
      expect(headerBox!.y).toBeGreaterThanOrEqual(0);
      await panel.hover();
      await page.mouse.wheel(0, 900);
      await expect.poll(() => panel.evaluate((el) => el.scrollTop)).toBeGreaterThan(0);
      expect((await header.boundingBox())!.y).toBe(headerBox!.y);
      expect((await navigation.boundingBox())!.y).toBe(navBox!.y);
      expect(await workspace.evaluate((el) => el.scrollTop)).toBe(0);
      expect(await navigation.evaluate((el) => el.scrollHeight <= el.clientHeight && el.scrollWidth <= el.clientWidth)).toBe(true);
      await panel.evaluate((el) => { el.scrollTop = el.scrollHeight; });
      await expect(panel.locator(".resource-row:visible").last()).toBeInViewport();
      expect((await header.boundingBox())!.y).toBe(headerBox!.y);
      expect(await page.locator(".app").evaluate((el) => el.scrollTop)).toBe(0);
      expect(await page.evaluate(() => scrollY)).toBe(0);
      await page.screenshot({ path: testInfo.outputPath(`${title}-scroll.png`), animations: "disabled" });
      if (title === "Ajustes") {
        await navigation.getByRole("button", { name: "Apariencia", exact: true }).click();
        await expect(workspace.getByRole("button", { name: "Usar tema claro" })).toBeInViewport();
        await workspace.getByRole("button", { name: "Cerrar ajustes" }).click();
        if (mobile) await page.getByRole("button", { name: "mostrar barra lateral" }).click();
        await page.getByRole("button", { name: "Conexiones", exact: true }).click();
        await page.getByRole("button", { name: "Servidores MCP", exact: true }).click();
      } else {
        await navigation.getByRole("button", { name: "Sistema de Ira", exact: true }).click();
        await expect(workspace.getByRole("heading", { name: "Sistema de Ira", exact: true })).toBeInViewport();
      }
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth && document.documentElement.scrollHeight <= innerHeight)).toBe(true);
  });
}

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
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  await page.getByRole("button", { name: "Bases de datos", exact: true }).click();
  await page.getByRole("button", { name: "Añadir base de datos" }).click();
  await page.getByLabel("URL de conexión PostgreSQL").fill("postgresql://reader:p%40ss@db.local:5433/analytics?sslmode=require");
  await page.getByRole("button", { name: "Revisar datos de URL" }).click();
  await page.getByText("Editar datos de conexión", { exact: true }).click();
  await expect(page.getByLabel("Servidor", { exact: true })).toHaveValue("db.local");
  await expect(page.getByLabel("Contraseña", { exact: true })).toHaveValue("p@ss");
  await expect(page.getByLabel("URL de conexión PostgreSQL")).toHaveValue("");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(page.getByRole("alert").filter({ hasText: "Falló la conexión" })).toBeVisible();
  await page.getByLabel("Servidor", { exact: true }).fill("db.correct.local");
  await page.getByRole("button", { name: "Guardar y comprobar" }).click();
  await expect(page.getByRole("status").filter({ hasText: "Conexión correcta" })).toBeVisible();
  expect(creates).toBe(1);
  expect(tests).toBe(2);
  expect(saved?.host).toBe("db.correct.local");
  expect(saved).not.toHaveProperty("password");
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Conexiones", exact: true })).toBeFocused();
});

test("settings remain alongside navigation and retain conversation draft", async ({ page }) => {
  await mockWorkspace(page);
  await page.goto("/");
  await page.getByRole("textbox", { name: "Mensaje para Ira" }).fill("Borrador sin enviar");
  await page.getByRole("button", { name: "Ajustes", exact: true }).click();
  const close = page.getByRole("region", { name: "Ajustes", exact: true }).getByRole("button", { name: "Cerrar ajustes", exact: true });
  await expect(close).toBeFocused();
  await page.keyboard.press("Shift+Tab");
  await expect(page.getByRole("button", { name: "Conexiones", exact: true })).toBeEnabled();
  await close.click();
  await expect(page.getByRole("textbox", { name: "Mensaje para Ira" })).toHaveValue("Borrador sin enviar");
  await page.getByRole("button", { name: "Elegir modelo", exact: true }).click();
  await page.getByLabel("Buscar modelo").fill("qwen");
  await expect(page.getByRole("button", { name: "Modelo en uso: qwen3:8b" })).toBeVisible();
});

test("external MCP is saved, automatically checked, and disabled in Connections", async ({ page }) => {
  await mockWorkspace(page);
  let server: Record<string, unknown> | null = null;
  await page.route("**/api/mcp**", async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path.endsWith("/test")) return route.fulfill({ json: { tools: ["list_tasks"] } });
    if (route.request().method() === "POST") {
      server = { ...route.request().postDataJSON(), id: "tasks", editable: true };
      return route.fulfill({ json: server });
    }
    return route.fulfill({ json: server ? [server] : [] });
  });
  await page.goto("/");
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  await page.getByRole("button", { name: "Servidores MCP", exact: true }).click();
  await page.getByRole("button", { name: "Añadir servidor MCP" }).click();
  await page.getByRole("textbox", { name: "Nombre del servidor MCP" }).fill("tasks");
  await page.getByRole("textbox", { name: "URL del servidor MCP" }).fill("https://example.com/custom/mcp");
  await page.getByRole("button", { name: "Guardar y comprobar", exact: true }).click();
  await expect(page.getByText("Conexión guardada.", { exact: false })).toBeVisible();
  expect(server).toMatchObject({ url: "https://example.com/custom/mcp", transport: "streamable_http" });
  await page.getByRole("button", { name: "Comprobar" }).click();
  await page.getByText("1 herramienta descubierta", { exact: true }).click();
  await expect(page.getByRole("region", { name: "Servidores MCP" })).toContainText("list_tasks");
  await page.screenshot({ path: "../.impeccable/review/mcp-desktop.png", animations: "disabled" });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.screenshot({ path: "../.impeccable/review/mcp-mobile.png", animations: "disabled" });
  await page.getByText("Detalles y acciones", { exact: true }).click();
  await page.getByRole("button", { name: "Desactivar" }).click();
  await expect(page.getByText("Desactivado", { exact: true })).toBeVisible();
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
  await expect(page.getByText("Preparando tu respuesta")).toBeVisible();
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
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  await page.getByRole("button", { name: "Añadir base de datos" }).click();
  await expect(page.getByLabel("URL de conexión PostgreSQL")).toBeVisible();
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
  await page.getByRole("button", { name: "Conexiones", exact: true }).click();
  await page.getByRole("button", { name: "Añadir base de datos" }).click();
  await expect(page.getByLabel("URL de conexión PostgreSQL")).toBeVisible();
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

test("phone and tablet keep composer, drawer and service controls usable without overflow", async ({ page }) => {
  await mockWorkspace(page);
  for (const width of [320, 390, 768]) {
    await page.setViewportSize({ width, height: 640 });
    await page.goto("/");
    await expect(page.getByRole("button", { name: "Permitir escritura" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Permitir escritura" })).toContainText("Escritura");
    await expect(page.getByRole("button", { name: "Enviar mensaje" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Elegir modelo", exact: true })).toBeVisible();
    const bar = page.locator(".stage form").first();
    expect(await bar.evaluate((el) => {
      const rect = el.getBoundingClientRect();
      return rect.left >= 0 && rect.right <= innerWidth && el.scrollWidth <= el.clientWidth + 1;
    })).toBe(true);
    await page.getByRole("button", { name: "mostrar barra lateral" }).click();
    await expect(page.getByRole("button", { name: "Documentación" })).toBeVisible();
    await page.getByRole("button", { name: "Conexiones", exact: true }).click();
    await page.getByRole("button", { name: "Sistema de Ira", exact: true }).click();
    await expect(page.getByRole("button", { name: "Iniciar servicios de arranque" })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  }
});
