import { Tooltip } from "@fluentui/react-components";
import { useState } from "react";
import { resourceData, useResources, bundledImage } from "../resources";
import { getLanguage, ui } from "../i18n";

const localized = () => {
  const language = getLanguage();
  return language === "zh-CN"
    ? undefined
    : (resourceData().names[language] as {
        champions: Record<string, string>;
        items: Record<string, string>;
      });
};
export const championName = (id: string) =>
  localized()?.champions[id] ??
  resourceData().catalog.champions[id]?.title ??
  id;
export const itemInfo = (id: number) => ({
  ...(resourceData().catalog.items[id] ?? { gold: 0, finished: false }),
  name:
    localized()?.items[id] ??
    resourceData().catalog.items[id]?.name ??
    `${ui("装备")} ${id}`,
});

function GameImage({
  src,
  label,
  size,
  className,
  fallback,
  localFallback,
}: {
  src?: string;
  label: string;
  size: number;
  className: string;
  fallback: string;
  localFallback?: string;
}) {
  const [failed, setFailed] = useState(false);
  const [local, setLocal] = useState(false);
  return !src || failed ? (
    <span
      className="asset-fallback"
      style={{ width: size, height: size }}
      role="img"
      aria-label={label}
      title={label}
    >
      {fallback}
    </span>
  ) : (
    <img
      className={className}
      src={local ? localFallback : src}
      width={size}
      height={size}
      alt={label}
      draggable={false}
      loading="lazy"
      referrerPolicy="no-referrer"
      onError={() => {
        if (localFallback && !local && src !== localFallback) setLocal(true);
        else setFailed(true);
      }}
    />
  );
}
const remote = (kind: "champion" | "item", id: string | number) =>
  `https://ddragon.leagueoflegends.com/cdn/${resourceData().catalog.version}/img/${kind}/${id}.png`;
export function Champion({ id, size = 40 }: { id: string; size?: number }) {
  useResources();
  const entry = resourceData().catalog.champions[id];
  return (
    <GameImage
      key={`${resourceData().catalog.version}:${id}`}
      src={
        entry
          ? entry.bundled
            ? `/assets/${id}.png`
            : remote("champion", id)
          : undefined
      }
      localFallback={bundledImage("champion", id)}
      label={championName(id)}
      size={size}
      className="champion"
      fallback={championName(id).slice(0, 2)}
    />
  );
}

export function Item({ id, size = 28 }: { id: number; size?: number }) {
  useResources();
  const entry = resourceData().catalog.items[id];
  return (
    <Tooltip content={itemInfo(id).name} relationship="label">
      <GameImage
        key={`${resourceData().catalog.version}:${id}`}
        src={
          entry
            ? entry.bundled
              ? `/assets/${id}.png`
              : remote("item", id)
            : undefined
        }
        localFallback={bundledImage("item", id)}
        label={itemInfo(id).name}
        size={size}
        className="item"
        fallback={String(id || "·")}
      />
    </Tooltip>
  );
}

export function ItemBuild({ items }: { items: number[] }) {
  return (
    <div className="item-build">
      {items.map((id, index) => (
        <Item key={index} id={id} />
      ))}
    </div>
  );
}
