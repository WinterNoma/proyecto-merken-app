import { defineConfig, loadEnv } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
import mysql from "mysql2/promise";

const host = process.env.TAURI_DEV_HOST;

/**
 * Las credenciales de MySQL viven solo en `server/.env` (ignorado por git; la plantilla
 * es `server/.env.example`). Solo se leen las variables `MYSQL_*`: el resto del archivo
 * (por ejemplo las de AWS) no se carga. No hay valores por defecto en el código.
 */
function mysqlSettings(mode: string) {
  const env = loadEnv(mode, `${process.cwd()}/server`, "MYSQL_");
  const sslMode = (env.MYSQL_SSL_MODE || "").toUpperCase();
  const required = ["MYSQL_HOST", "MYSQL_USER", "MYSQL_PASSWORD", "MYSQL_DATABASE"] as const;
  const missing = required.filter(name => !env[name]);
  if (missing.length > 0) {
    throw new Error(`MySQL is not configured: set ${missing.join(", ")} in server/.env`);
  }
  return {
    host: env.MYSQL_HOST,
    port: Number(env.MYSQL_PORT) || 3306,
    user: env.MYSQL_USER,
    password: env.MYSQL_PASSWORD,
    database: env.MYSQL_DATABASE,
    // Con MYSQL_SSL_MODE=REQUIRED se cifra la conexión (sin verificar el certificado del servidor).
    ssl: sslMode === "REQUIRED" ? { rejectUnauthorized: false } : undefined,
    // Si MySQL no responde, /api/astronauts devuelve un 500 claro en vez de quedarse colgado.
    connectTimeout: 10000,
    waitForConnections: true,
    connectionLimit: 4,
    queueLimit: 0,
  };
}

// https://vite.dev/config/
export default defineConfig(({ mode }) => {
  // El pool se crea la primera vez que se pide, para que una configuración que falta
  // se vea como un error 500 claro en /api/astronauts y no tumbe el servidor de Vite.
  let pool: ReturnType<typeof mysql.createPool> | null = null;
  const dbPool = {
    query: (sql: string) => {
      pool ??= mysql.createPool(mysqlSettings(mode));
      return pool.query(sql);
    },
  };
  return {
    // La interfaz vive en ui/; Tauri lee el build desde dist/ en la raíz.
    root: "ui",
    build: {
      outDir: "../dist",
      emptyOutDir: true,
    },

    plugins: [
      {
        name: "mysql-api-middleware",
        configureServer(server) {
          server.middlewares.use(async (req, res, next) => {
            if (req.url === "/api/astronauts" || req.url?.startsWith("/api/astronauts?")) {
              try {
                const [rows] = await dbPool.query("SELECT * FROM astronauts ORDER BY id ASC");
                res.setHeader("Content-Type", "application/json");
                res.setHeader("Cache-Control", "no-cache");
                res.end(JSON.stringify(rows));
              } catch (err: unknown) {
                res.statusCode = 500;
                res.setHeader("Content-Type", "application/json");
                res.end(JSON.stringify({ error: String(err) }));
              }
              return;
            }
            next();
          });
        },
      },
    ],

    // Opciones para Tauri (`tauri dev` / `tauri build`)
    clearScreen: false,
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        ignored: ["**/src-tauri/**", "**/crates/**"],
      },
    },
  };
});
