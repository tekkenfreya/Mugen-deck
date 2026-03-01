import type { FastifyInstance } from "fastify";

/** Hardcoded app version data for Phase 1. */
const APP_VERSIONS: Record<string, { version: string; downloadUrl: string }> = {
  sharkdeck: {
    version: "0.1.0",
    downloadUrl: "https://cdn.mugen.gg/apps/sharkdeck/0.1.0/sharkdeck.tar.gz",
  },
};

const getAppLatestSchema = {
  params: {
    type: "object" as const,
    properties: {
      id: { type: "string" as const },
    },
    required: ["id" as const],
  },
};

export async function appRoutes(app: FastifyInstance) {
  app.get<{ Params: { id: string } }>(
    "/api/v1/apps/:id/latest",
    { schema: getAppLatestSchema },
    async (request, reply) => {
      const { id } = request.params;
      const appData = APP_VERSIONS[id];

      if (!appData) {
        return reply.status(404).send({
          ok: false,
          error: `app '${id}' not found`,
        });
      }

      return {
        ok: true,
        data: {
          id,
          ...appData,
        },
      };
    },
  );
}
