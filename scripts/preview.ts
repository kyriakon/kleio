import path from "path";
import { fileURLToPath } from "url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.join(__dirname, "..");
const distDir = path.join(projectRoot, "dist");
const port = Number(process.env.PORT || 4173);

function getFilePath(urlPath: string) {
  let pathname = urlPath;
  if (pathname === "/") pathname = "/index.html";
  if (pathname.endsWith("/")) pathname += "index.html";
  return path.join(distDir, pathname.replace(/^[\/]+/, ""));
}

const server = Bun.serve({
  port,
  fetch(req) {
    const filePath = getFilePath(new URL(req.url).pathname);
    try {
      return new Response(Bun.file(filePath));
    } catch {
      return new Response("Not Found", { status: 404 });
    }
  },
});

console.log(`Preview server running at http://localhost:${server.port}`);
await new Promise(() => {});
