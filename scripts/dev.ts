import * as fs from "fs";
import fsPromises from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.join(__dirname, "..");
const distDir = path.join(projectRoot, "dist");
const publicDir = path.join(projectRoot, "public");
const indexHtml = path.join(projectRoot, "index.html");
const entry = path.join(projectRoot, "src/main.tsx");
const bunExec = process.execPath;

async function copyRecursive(src: string, dest: string) {
  const stats = await fsPromises.stat(src);
  if (stats.isDirectory()) {
    await fsPromises.mkdir(dest, { recursive: true });
    for (const entry of await fsPromises.readdir(src)) {
      await copyRecursive(path.join(src, entry), path.join(dest, entry));
    }
  } else if (stats.isFile()) {
    await fsPromises.mkdir(path.dirname(dest), { recursive: true });
    await fsPromises.copyFile(src, dest);
  }
}

async function copyPublic() {
  await fsPromises.mkdir(distDir, { recursive: true });
  await fsPromises.copyFile(indexHtml, path.join(distDir, "index.html"));

  try {
    const stats = await fsPromises.stat(publicDir);
    if (stats.isDirectory()) {
      await copyRecursive(publicDir, distDir);
    }
  } catch {
    // Ignore if the public directory does not exist.
  }
}

function serve() {
  return Bun.serve({
    port: 1420,
    async fetch(req) {
      const url = new URL(req.url);
      let pathname = url.pathname;
      if (pathname === "/") pathname = "/index.html";
      if (pathname.endsWith("/")) pathname += "index.html";
      const filePath = path.join(distDir, pathname.replace(/^[\/]+/, ""));
      try {
        return new Response(Bun.file(filePath));
      } catch {
        return new Response("Not Found", { status: 404 });
      }
    },
  });
}

async function main() {
  await copyPublic();

  const server = serve();
  console.log(`Dev server running at http://localhost:${server.port}`);

  try {
    const indexWatcher = Bun.watch(indexHtml, { recursive: false }, async () => {
      await copyPublic();
    });
    process.on("exit", () => indexWatcher.close());
  } catch {
    fs.watch(indexHtml, async () => {
      await copyPublic();
    });
  }

  try {
    const publicWatcher = Bun.watch(publicDir, { recursive: true }, async () => {
      await copyPublic();
    });
    process.on("exit", () => publicWatcher.close());
  } catch {
    fs.watch(publicDir, { recursive: true }, async () => {
      await copyPublic();
    });
  }

  const cssBuilder = Bun.spawn({
    cmd: [bunExec, "x", "tailwindcss", "-c", "tailwind.config.cjs", "-i", path.join(projectRoot, "src/styles.css"), "-o", path.join(distDir, "main.css"), "--watch"],
    cwd: projectRoot,
    stdout: "inherit",
    stderr: "inherit",
  });

  const jsBuilder = Bun.spawn({
    cmd: [bunExec, "build", "--watch", "--outdir", distDir, entry],
    cwd: projectRoot,
    stdout: "inherit",
    stderr: "inherit",
  });

  process.on("SIGINT", () => {
    cssBuilder.kill("SIGINT");
    jsBuilder.kill("SIGINT");
    process.exit(0);
  });

  await Promise.all([cssBuilder.exited, jsBuilder.exited]);
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
