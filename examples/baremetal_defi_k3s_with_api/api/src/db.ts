import { createClient, type ClickHouseClient } from "@clickhouse/client";

const DB_HOST = process.env.DB_HOST ?? "http://localhost:8123";
const DB_USER = process.env.DB_USER ?? "default";
const DB_PASSWORD = process.env.DB_PASSWORD ?? "";
const DB_NAME = process.env.DB_NAME ?? "rindexer";

let client: ClickHouseClient | null = null;

export function getClient(): ClickHouseClient {
  if (!client) {
    client = createClient({ url: DB_HOST, username: DB_USER, password: DB_PASSWORD, database: DB_NAME });
  }
  return client;
}
