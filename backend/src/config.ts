import "dotenv/config";

export interface Config {
  port: number;
  host: string;
  nodeEnv: string;
  corsOrigins: string[];
}

export function loadConfig(): Config {
  const nodeEnv = process.env["NODE_ENV"] ?? "development";
  const isDev = nodeEnv === "development";

  const corsOrigins = [
    "https://mugen.gg",
    "https://cheatcode.dev",
  ];

  if (isDev) {
    corsOrigins.push("http://localhost:3000");
    corsOrigins.push("http://localhost:1420");
  }

  return {
    port: parseInt(process.env["PORT"] ?? "3000", 10),
    host: process.env["HOST"] ?? "0.0.0.0",
    nodeEnv,
    corsOrigins,
  };
}
