import { Router, type Request, type Response } from "express";
import { z } from "zod";
import { getDb, save, lastInsertId } from "../db.js";

const router = Router();

const createSchema = z.object({
  name: z.string().min(1),
  description: z.string().optional(),
});

const updateSchema = z.object({
  name: z.string().min(1).optional(),
  description: z.string().optional(),
});

router.get("/", async (_req: Request, res: Response) => {
  const db = await getDb();
  const stmt = db.prepare(`
    SELECT p.*, COUNT(t.id) as task_count
    FROM projects p
    LEFT JOIN tasks t ON t.project_id = p.id
    GROUP BY p.id
    ORDER BY p.created_at DESC
  `);
  const rows: Record<string, any>[] = [];
  while (stmt.step()) rows.push(stmt.getAsObject());
  stmt.free();
  res.json(rows);
});

router.get("/:id", async (req: Request, res: Response) => {
  const db = await getDb();
  const stmt = db.prepare(`
    SELECT p.*, COUNT(t.id) as task_count
    FROM projects p
    LEFT JOIN tasks t ON t.project_id = p.id
    WHERE p.id = ?
    GROUP BY p.id
  `);
  stmt.bind([String(req.params.id)]);
  const row = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  if (!row) return res.status(404).json({ error: "Project not found" });
  res.json(row);
});

router.post("/", async (req: Request, res: Response) => {
  const parsed = createSchema.safeParse(req.body);
  if (!parsed.success) return res.status(400).json({ errors: parsed.error.flatten() });

  const db = await getDb();
  const { name, description } = parsed.data;
  db.run("INSERT INTO projects (name, description) VALUES (?, ?)", [name, description ?? ""]);
  const id = lastInsertId();
  save();
  const stmt = db.prepare("SELECT * FROM projects WHERE id = ?");
  stmt.bind([id]);
  const row = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  res.status(201).json(row);
});

router.put("/:id", async (req: Request, res: Response) => {
  const parsed = updateSchema.safeParse(req.body);
  if (!parsed.success) return res.status(400).json({ errors: parsed.error.flatten() });

  const db = await getDb();
  const id = String(req.params.id);
  const stmt = db.prepare("SELECT * FROM projects WHERE id = ?");
  stmt.bind([id]);
  const existing = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  if (!existing) return res.status(404).json({ error: "Project not found" });

  const { name, description } = parsed.data;
  db.run(`
    UPDATE projects SET name = COALESCE(?, name), description = COALESCE(?, description), updated_at = datetime('now')
    WHERE id = ?
  `, [name ?? null, description ?? null, id]);
  save();

  const stmt2 = db.prepare("SELECT * FROM projects WHERE id = ?");
  stmt2.bind([id]);
  const row = stmt2.step() ? stmt2.getAsObject() : null;
  stmt2.free();
  res.json(row);
});

router.delete("/:id", async (req: Request, res: Response) => {
  const db = await getDb();
  const id = String(req.params.id);
  const stmt = db.prepare("SELECT * FROM projects WHERE id = ?");
  stmt.bind([id]);
  const existing = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  if (!existing) return res.status(404).json({ error: "Project not found" });

  db.run("DELETE FROM projects WHERE id = ?", [id]);
  save();
  res.status(204).send();
});

export default router;
