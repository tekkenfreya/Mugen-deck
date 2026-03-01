import Fastify from "fastify";
import cors from "@fastify/cors";
import { loadConfig } from "./config.js";
import { healthRoutes } from "./routes/health.js";
import { appRoutes } from "./routes/apps.js";

const config = loadConfig();

const app = Fastify({
  logger: {
    level: config.nodeEnv === "production" ? "info" : "debug",
  },
});

// CORS
await app.register(cors, {
  origin: config.corsOrigins,
});

// Routes
await app.register(healthRoutes);
await app.register(appRoutes);

// Start
try {
  await app.listen({ port: config.port, host: config.host });
} catch (err) {
  app.log.error(err);
  process.exit(1);
}
