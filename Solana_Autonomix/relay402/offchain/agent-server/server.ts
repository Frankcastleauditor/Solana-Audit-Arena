import express, { NextFunction, Request, Response } from "express";
import { PAYMENT_RESPONSE_HEADER } from "../shared/x402";
import { PaywallOptions, createPaywall } from "./paywall";
import { MAX_PROMPT_LEN, parseTaskInput, runTask } from "./task";

export interface AgentAppOptions extends PaywallOptions {
  /** Exact bytes of the metadata document. Its sha256 is stored on-chain. */
  metadata: Buffer;
}

export function createAgentApp(opts: AgentAppOptions): express.Express {
  const app = express();
  app.disable("x-powered-by");

  // Let browser clients read the settlement header.
  app.use((_req, res, next) => {
    res.setHeader("Access-Control-Expose-Headers", PAYMENT_RESPONSE_HEADER);
    next();
  });

  app.get("/metadata.json", (_req, res) => {
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.type("application/json").send(opts.metadata);
  });

  const paywall = createPaywall(opts, {
    parse: parseTaskInput,
    run: (input) => runTask(input, opts.taskTimeoutMs),
  });

  // Raw body: the client signs sha256(body bytes), so we must hash exactly
  // what was sent, before any JSON re-serialization.
  app.post(
    "/api/run",
    express.raw({ type: () => true, limit: MAX_PROMPT_LEN * 4 + 1024 }),
    (req, res, next) => {
      paywall(req, res).catch(next);
    },
  );

  app.use((err: unknown, _req: Request, res: Response, _next: NextFunction) => {
    const status = (err as { status?: number }).status;
    if (status === 400 || status === 413) {
      res.status(status).json({ error: "bad_request" });
      return;
    }
    console.error("[agent] unhandled error:", err);
    if (!res.headersSent) res.status(500).json({ error: "internal_error" });
  });

  return app;
}
