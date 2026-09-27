import { Router, type Request, type Response } from "express";
import { z } from "zod";
import { getDb, save, lastInsertId } from "../db.js";

const router = Router({ mergeParams: true });

const createSchema = z.object({
  title: z.string().min(1),
  description: z.string().optional(),
  status: z.enum(["todo", "in_progress", "done"]).optional(),
  priority: z.enum(["low", "medium", "high"]).optional(),
});

const updateSchema = z.object({
  title: z.string().min(1).optional(),
  description: z.string().optional(),
  status: z.enum(["todo", "in_progress", "done"]).optional(),
  priority: z.enum(["low", "medium", "high"]).optional(),
});

router.get("/", async (req: Request, res: Response) => {
  const db = await getDb();
  const projectId = String(req.params.projectId);
  const stmt = db.prepare("SELECT * FROM tasks WHERE project_id = ? ORDER BY created_at DESC");
  stmt.bind([projectId]);
  const rows: Record<string, any>[] = [];
  while (stmt.step()) rows.push(stmt.getAsObject());
  stmt.free();
  res.json(rows);
});

router.get("/:taskId", async (req: Request, res: Response) => {
  const db = await getDb();
  const taskId = String(req.params.taskId);
  const projectId = String(req.params.projectId);
  const stmt = db.prepare("SELECT * FROM tasks WHERE id = ? AND project_id = ?");
  stmt.bind([taskId, projectId]);
  const row = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  if (!row) return res.status(404).json({ error: "Task not found" });
  res.json(row);
});

router.post("/", async (req: Request, res: Response) => {
  const db = await getDb();
  const projectId = String(req.params.projectId);

  const stmtP = db.prepare("SELECT id FROM projects WHERE id = ?");
  stmtP.bind([projectId]);
  const project = stmtP.step() ? stmtP.getAsObject() : null;
  stmtP.free();
  if (!project) return res.status(404).json({ error: "Project not found" });

  const parsed = createSchema.safeParse(req.body);
  if (!parsed.success) return res.status(400).json({ errors: parsed.error.flatten() });

  const { title, description, status, priority } = parsed.data;
  db.run(
    "INSERT INTO tasks (project_id, title, description, status, priority) VALUES (?, ?, ?, ?, ?)",
    [projectId, title, description ?? "", status ?? "todo", priority ?? "medium"]
  );
  const id = lastInsertId();
  save();
  const stmt = db.prepare("SELECT * FROM tasks WHERE id = ?");
  stmt.bind([id]);
  const row = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  res.status(201).json(row);
});

router.put("/:taskId", async (req: Request, res: Response) => {
  const db = await getDb();
  const projectId = String(req.params.projectId);
  const taskId = String(req.params.taskId);

  const stmtE = db.prepare("SELECT * FROM tasks WHERE id = ? AND project_id = ?");
  stmtE.bind([taskId, projectId]);
  const existing = stmtE.step() ? stmtE.getAsObject() : null;
  stmtE.free();
  if (!existing) return res.status(404).json({ error: "Task not found" });

  const parsed = updateSchema.safeParse(req.body);
  if (!parsed.success) return res.status(400).json({ errors: parsed.error.flatten() });

  const { title, description, status, priority } = parsed.data;
  db.run(`
    UPDATE tasks
    SET title = COALESCE(?, title),
        description = COALESCE(?, description),
        status = COALESCE(?, status),
        priority = COALESCE(?, priority),
        updated_at = datetime('now')
    WHERE id = ? AND project_id = ?
  `, [title ?? null, description ?? null, status ?? null, priority ?? null, taskId, projectId]);
  save();

  const stmt = db.prepare("SELECT * FROM tasks WHERE id = ?");
  stmt.bind([taskId]);
  const row = stmt.step() ? stmt.getAsObject() : null;
  stmt.free();
  res.json(row);
});

router.delete("/:taskId", async (req: Request, res: Response) => {
  const db = await getDb();
  const projectId = String(req.params.projectId);
  const taskId = String(req.params.taskId);

  const stmtE = db.prepare("SELECT * FROM tasks WHERE id = ? AND project_id = ?");
  stmtE.bind([taskId, projectId]);
  const existing = stmtE.step() ? stmtE.getAsObject() : null;
  stmtE.free();
  if (!existing) return res.status(404).json({ error: "Task not found" });

  db.run("DELETE FROM tasks WHERE id = ? AND project_id = ?", [taskId, projectId]);
  save();
  res.status(204).send();
});

export default router;
