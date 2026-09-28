/**
 * The paid work. Input is validated BEFORE any payment step so a client
 * never pays for a request the agent would reject.
 */
export interface TaskInput {
  prompt: string;
}

export interface TaskOutput {
  output: string;
  model: string;
}

export const MAX_PROMPT_LEN = 4000;

export function parseTaskInput(raw: Buffer): TaskInput {
  let body: unknown;
  try {
    body = JSON.parse(raw.toString("utf8"));
  } catch {
    throw new Error("body must be JSON");
  }
  if (typeof body !== "object" || body === null) throw new Error("body must be a JSON object");
  const prompt = (body as { prompt?: unknown }).prompt;
  if (typeof prompt !== "string" || prompt.trim().length === 0 || prompt.length > MAX_PROMPT_LEN) {
    throw new Error(`prompt must be a non-empty string up to ${MAX_PROMPT_LEN} chars`);
  }
  return { prompt };
}

/**
 * Calls the Anthropic Messages API when ANTHROPIC_API_KEY is set, otherwise
 * returns a deterministic stub (used by tests and local demos).
 */
export async function runTask(input: TaskInput, timeoutMs: number): Promise<TaskOutput> {
  const apiKey = process.env.ANTHROPIC_API_KEY;
  if (!apiKey) {
    const words = input.prompt.trim().split(/\s+/).length;
    return { output: `stub agent: received ${words} word(s)`, model: "stub" };
  }

  const model = process.env.AGENT_MODEL || "claude-sonnet-5";
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const res = await fetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: {
        "content-type": "application/json",
        "x-api-key": apiKey,
        "anthropic-version": "2023-06-01",
      },
      body: JSON.stringify({
        model,
        max_tokens: 1000,
        messages: [{ role: "user", content: input.prompt }],
      }),
      signal: controller.signal,
    });
    if (!res.ok) throw new Error(`model API returned ${res.status}`);
    const data = (await res.json()) as { content?: Array<{ type: string; text?: string }> };
    const output = (data.content ?? [])
      .filter((b) => b.type === "text" && typeof b.text === "string")
      .map((b) => b.text)
      .join("\n");
    return { output, model };
  } finally {
    clearTimeout(timer);
  }
}
