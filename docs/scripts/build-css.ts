const docsRoot = new URL("../", import.meta.url);
const docsBaseCss = new URL("../css/base.css", import.meta.url);
const docsOutputCss = new URL("../static/css/style.css", import.meta.url);
const sharedSourceCss = new URL("../../crates/ksbh-ui/static/css/shared.css", import.meta.url);
const sharedOutputCss = new URL("../../crates/ksbh-ui/static/css/style.css", import.meta.url);

const watchMode = Deno.args.includes("--watch");

async function runTailwind(input: URL, output: URL) {
  const cmd = new Deno.Command("deno", {
    args: [
      "run",
      "-A",
      "npm:@tailwindcss/cli",
      "-i",
      input.pathname,
      "-o",
      output.pathname,
    ],
    cwd: docsRoot,
    stdout: "inherit",
    stderr: "inherit",
  });

  const result = await cmd.output();
  if (!result.success) {
    throw new Error(`tailwind build failed for ${input.pathname}`);
  }
}

async function buildAll() {
  await runTailwind(sharedSourceCss, sharedOutputCss);
  await runTailwind(docsBaseCss, docsOutputCss);
}

if (!watchMode) {
  await buildAll();
  Deno.exit(0);
}

console.log("[dev:css] initial build");
await buildAll();

const watcher = Deno.watchFs([
  docsBaseCss,
  sharedSourceCss,
]);

let pending = false;
let scheduled = false;

async function rebuild() {
  if (pending) {
    scheduled = true;
    return;
  }

  pending = true;
  try {
    console.log("[dev:css] rebuilding");
    await buildAll();
    console.log("[dev:css] ready");
  } catch (error) {
    console.error("[dev:css] build failed", error);
  } finally {
    pending = false;
    if (scheduled) {
      scheduled = false;
      queueMicrotask(() => {
        void rebuild();
      });
    }
  }
}

for await (const _event of watcher) {
  void rebuild();
}
