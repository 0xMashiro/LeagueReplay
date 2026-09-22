import { readFile, writeFile } from "node:fs/promises";
const catalog = JSON.parse(
  await readFile(new URL("../src/data/catalog.json", import.meta.url), "utf8"),
);
const languages = { en: "en_US", ja: "ja_JP", ko: "ko_KR" };
const names = {};
await Promise.all(
  Object.entries(languages).map(async ([language, locale]) => {
    const [champions, items] = await Promise.all(
      ["champion", "item"].map(async (kind) => {
        const response = await fetch(
          `https://ddragon.leagueoflegends.com/cdn/${catalog.version}/data/${locale}/${kind}.json`,
          { signal: AbortSignal.timeout(30000) },
        );
        if (!response.ok)
          throw new Error(`${locale} ${kind}: ${response.status}`);
        return (await response.json()).data;
      }),
    );
    names[language] = {
      champions: Object.fromEntries(
        Object.entries(champions).map(([id, value]) => [id, value.name]),
      ),
      items: Object.fromEntries(
        Object.entries(items).map(([id, value]) => [id, value.name]),
      ),
    };
  }),
);
await writeFile(
  new URL("../src/data/asset-names.json", import.meta.url),
  JSON.stringify({ version: catalog.version, ...names }) + "\n",
);
console.log(`Saved official EN/JA/KO names for ${catalog.version}.`);
