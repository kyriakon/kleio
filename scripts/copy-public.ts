import fs from "fs/promises";
import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.join(__dirname, "..");
const distDir = path.join(projectRoot, "dist");
const publicDir = path.join(projectRoot, "public");
const indexHtml = path.join(projectRoot, "index.html");

async function copyRecursive(src: string, dest: string) {
  const stats = await fs.stat(src);
  if (stats.isDirectory()) {
    await fs.mkdir(dest, { recursive: true });
    for (const entry of await fs.readdir(src)) {
      await copyRecursive(path.join(src, entry), path.join(dest, entry));
    }
  } else if (stats.isFile()) {
    await fs.mkdir(path.dirname(dest), { recursive: true });
    await fs.copyFile(src, dest);
  }
}

async function main() {
  await fs.mkdir(distDir, { recursive: true });
  await fs.copyFile(indexHtml, path.join(distDir, "index.html"));

  try {
    const stats = await fs.stat(publicDir);
    if (stats.isDirectory()) {
      await copyRecursive(publicDir, distDir);
    }
  } catch {
    // Ignore if the public directory does not exist.
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
