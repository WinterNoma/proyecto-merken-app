import { defineConfig } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
import mysql from "mysql2/promise";

const host = process.env.TAURI_DEV_HOST;

const dbPool = mysql.createPool({
  host: process.env.MYSQL_HOST || "98.95.196.77", //RECORDAR QUITAR ETA WEA Y MOVERLO AL ENV(DA COMO QUE IGUAL PERO IMPORTANTE)
  port: Number(process.env.MYSQL_PORT) || 3306,
  user: process.env.MYSQL_USER || "merken_user",
  password: process.env.MYSQL_PASSWORD || "merkenlomasgrande1234",
  database: process.env.MYSQL_DATABASE || "merken_db",
  waitForConnections: true,
  connectionLimit: 4,
  queueLimit: 0,
});

// https://vite.dev/config/
export default defineConfig(() => ({
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
}));
