import express, { NextFunction, Request, Response } from "express";
import { Facilitator } from "./facilitator";

export function createFacilitatorApp(facilitator: Facilitator): express.Express {
  const app = express();
  app.disable("x-powered-by");
  app.use(express.json({ limit: "16kb" }));

  app.get("/supported", (_req, res) => {
    res.json(facilitator.supported());
  });

  app.post("/verify", async (req, res, next) => {
    try {
      const out = await facilitator.verify(req.body);
      res.status(out.isValid ? 200 : 400).json(out);
    } catch (e) {
      next(e);
    }
  });

  app.post("/settle", async (req, res, next) => {
    try {
      const out = await facilitator.settle(req.body);
      res.status(out.success ? 200 : 400).json(out);
    } catch (e) {
      next(e);
    }
  });

  // Never leak stack traces to callers.
  app.use((err: unknown, _req: Request, res: Response, _next: NextFunction) => {
    const status = (err as { status?: number }).status;
    if (status === 400 || status === 413) {
      res.status(status).json({ error: "bad_request" });
      return;
    }
    console.error("[facilitator] unhandled error:", err);
    res.status(500).json({ error: "internal_error" });
  });

  return app;
}
