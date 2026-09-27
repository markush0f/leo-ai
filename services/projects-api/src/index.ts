import express from "express";
import cors from "cors";
import { getDb } from "./db.js";
import projectsRouter from "./routes/projects.js";
import tasksRouter from "./routes/tasks.js";

const app = express();
const port = parseInt(process.env.PORT || "3200");

app.use(cors());
app.use(express.json());

app.use("/projects", projectsRouter);
app.use("/projects/:projectId/tasks", tasksRouter);

app.get("/health", (_req, res) => {
  res.json({ status: "ok" });
});

getDb().then(() => {
  app.listen(port, () => {
    console.log(`projects-api running on http://localhost:${port}`);
  });
});
