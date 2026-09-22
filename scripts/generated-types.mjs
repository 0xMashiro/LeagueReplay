import { spawnSync } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  writeFile,
  rm,
} from "node:fs/promises";
import { dirname, join, resolve, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const update = process.argv.includes("--write");
const cache = join(root, ".cache");
await mkdir(cache, { recursive: true });
const temporary = await mkdtemp(join(cache, "type-bindings-"));

async function files(directory, prefix = "") {
  const result = new Map();
  let entries;
  try {
    entries = await readdir(join(directory, prefix), { withFileTypes: true });
  } catch (error) {
    if (error.code === "ENOENT") return result;
    throw error;
  }
  for (const entry of entries) {
    const name = join(prefix, entry.name);
    if (entry.isDirectory()) {
      for (const [path, content] of await files(directory, name))
        result.set(path, content);
    } else if (entry.isFile()) {
      result.set(
        name,
        (await readFile(join(directory, name), "utf8")).replaceAll(
          "\r\n",
          "\n",
        ),
      );
    } else {
      throw new Error(`Unexpected generated entry: ${name}`);
    }
  }
  return result;
}

const bindingRoots = [join("src", "generated"), join("src-tauri", "bindings")];
async function bindings(workspace) {
  const result = new Map();
  for (const directory of bindingRoots) {
    for (const [name, content] of await files(join(workspace, directory))) {
      result.set(join(directory, name), content);
    }
  }
  return result;
}

try {
  // export_to paths are relative to the ts-rs base; keep their ../../ inside this temporary tree.
  const base = join(temporary, "src-tauri", "bindings");
  await mkdir(base, { recursive: true });
  const cargo = spawnSync(
    "cargo",
    [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--locked",
      "--workspace",
      "--target",
      "x86_64-pc-windows-msvc",
      "export_bindings",
    ],
    {
      cwd: root,
      stdio: "inherit",
      env: { ...process.env, TS_RS_EXPORT_DIR: base },
    },
  );
  if (cargo.error) throw cargo.error;
  if (cargo.status !== 0) throw new Error("Rust type export failed.");
  const generated = await bindings(temporary);
  if (!generated.size) throw new Error("Rust exported no bindings.");
  const existing = await bindings(root);
  const names = [...new Set([...existing.keys(), ...generated.keys()])].sort();
  const changed = names.filter(
    (name) => existing.get(name) !== generated.get(name),
  );
  if (update) {
    for (const name of changed) {
      const destination = resolve(root, name);
      if (
        !bindingRoots.some((directory) =>
          destination.startsWith(join(root, directory) + sep),
        )
      )
        throw new Error("Invalid generated path.");
      if (!generated.has(name)) await rm(destination);
      else {
        await mkdir(dirname(destination), { recursive: true });
        await writeFile(destination, generated.get(name));
      }
    }
    console.log(`Updated ${changed.length} generated files.`);
  } else if (changed.length) {
    console.error(
      `Generated types have drifted:\n${changed.map((name) => `  ${name}`).join("\n")}\nRun npm run types:generate.`,
    );
    process.exitCode = 1;
  } else {
    console.log(`Verified ${generated.size} generated files.`);
  }
} finally {
  const child = relative(cache, temporary);
  if (child.startsWith("type-bindings-") && !child.includes(sep)) {
    await rm(temporary, { recursive: true, force: true });
  } else {
    console.error("Invalid temporary directory; cleanup skipped.");
    process.exitCode = 1;
  }
}
