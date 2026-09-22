import { mkdir, writeFile } from "node:fs/promises";

// Riot Games Data Dragon artwork is not covered by this project's source-code license.
const explicitVersion = process.argv.find(arg=>arg.startsWith("--version="))?.slice(10);
const version = explicitVersion ?? (await (await fetch("https://ddragon.leagueoflegends.com/api/versions.json")).json())[0];
if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("Invalid Data Dragon version");
const root = new URL("../public/assets/", import.meta.url);
const champions = [
  "Ahri",
  "Orianna",
  "Sylas",
  "Hwei",
  "Camille",
  "Vi",
  "Jinx",
  "Nautilus",
  "Aatrox",
  "LeeSin",
  "Syndra",
  "Ezreal",
  "Leona",
];
const items = [
  1056, 1055, 1054, 2003, 2055, 1001, 1004, 1036, 1026, 1052, 1082, 1028, 1037,
  1042, 1043, 1029, 1033, 3006, 3020, 3047, 3111, 3118, 6655, 3089, 3135, 3157,
  4645, 3040, 3031, 3085, 3072, 3036, 3078, 3053, 6333, 3071, 3026, 3190, 3107,
  3050, 3068,
];
async function get(url) {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`${response.status}: ${url}`);
  return response;
}
await mkdir(root, { recursive: true });
const itemData = await (
  await get(
    `https://ddragon.leagueoflegends.com/cdn/${version}/data/zh_CN/item.json`,
  )
).json();
const championData = await (
  await get(
    `https://ddragon.leagueoflegends.com/cdn/${version}/data/zh_CN/champion.json`,
  )
).json();
const catalog = { version, champions: {}, items: {} };
for (const champion of Object.keys(championData.data)) {
  const record = championData.data[champion];
  catalog.champions[champion] = {
    key: Number(record.key),
    name: record.name,
    title: record.title,
    bundled: champions.includes(champion),
  };
}
for (const id of Object.keys(itemData.data)) {
  const item = itemData.data[id];
  if (!item) throw new Error(`Missing item ${id}`);
  catalog.items[id] = {
    name: item.name,
    bundled: items.includes(Number(id)),
    gold: item.gold.total,
    finished:
      (item.from?.length > 0 && !item.into?.length) || item.gold.total >= 2200,
  };
}
const jobs = [
  ...champions.map((id) => ({
    file: `${id}.png`,
    url: `https://ddragon.leagueoflegends.com/cdn/${version}/img/champion/${id}.png`,
  })),
  ...items.map((id) => ({
    file: `${id}.png`,
    url: `https://ddragon.leagueoflegends.com/cdn/${version}/img/item/${id}.png`,
  })),
];
for (
  let i = 0;
  !process.argv.includes("--catalog-only") && i < jobs.length;
  i += 6
) {
  await Promise.all(
    jobs.slice(i, i + 6).map(async (job) => {
      const bytes = await (await get(job.url)).arrayBuffer();
      await writeFile(new URL(job.file, root), Buffer.from(bytes));
    }),
  );
}
await writeFile(
  new URL("../src/data/catalog.json", import.meta.url),
  JSON.stringify(catalog, null, 2) + "\n",
);
console.log(
  `Saved catalog (${version}), ${Object.keys(catalog.champions).length} champions, ${Object.keys(catalog.items).length} items. Images ${process.argv.includes("--catalog-only") ? "unchanged" : "updated"}.`,
);
