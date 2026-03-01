import type { FastifyInstance } from "fastify";

export async function healthRoutes(app: FastifyInstance) {
  app.get("/api/v1/health", async () => {
    return {
      ok: true,
      data: {
        status: "running",
        version: "0.1.0",
      },
    };
  });
}
