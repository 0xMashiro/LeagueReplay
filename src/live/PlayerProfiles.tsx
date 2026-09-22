import { useState } from "react";
import { Button } from "@fluentui/react-components";
import { People20Regular } from "@fluentui/react-icons";
import { ui, useI18n } from "../i18n";
import { knownPlayerNames } from "./adapter";
import { SearchField } from "../components/SearchField";
import { ProfileTools } from "./ProfileTools";
import { PlayerHistoryLink } from "./PlayerHistoryLink";
import type { LiveStore } from "./useLiveWorkspace";
import type { Account } from "../generated/live/Account";

export function PlayerProfiles({
  store,
  accounts,
}: {
  store: LiveStore;
  accounts: Account[];
}) {
  const { t, date } = useI18n();
  const [profileQuery, setProfileQuery] = useState("");
  const view = store.workspace!;
  const accountById = Object.fromEntries(
    [...view.accounts, ...accounts].map((a) => [a.id, a]),
  );
  const playerNames = knownPlayerNames(view.matches);
  const profiles = view.ui.players.filter((profile) =>
    [
      profile.name,
      ...profile.accountIds.map(
        (id) => playerNames[id] ?? accountById[id]?.riotId ?? id,
      ),
      ...(view.ui.identityLinks ?? [])
        .filter((link) => link.playerId === profile.id)
        .map(
          (link) =>
            playerNames[link.accountId] ??
            accountById[link.accountId]?.riotId ??
            link.accountId,
        ),
    ]
      .join(" ")
      .toLocaleLowerCase()
      .includes(profileQuery.trim().toLocaleLowerCase()),
  );
  return (
    <div className="profiles-page">
      {!!view.ui.players.length && (
        <SearchField
          className="notes-search"
          label={t("profileSearch")}
          value={profileQuery}
          onChange={setProfileQuery}
        />
      )}
      {profiles.length ? (
        profiles.map((profile) => (
          <section className="live-profile" key={profile.id}>
            <div className="profile-heading">
              <div className="profile-identity">
                <span className="profile-mark" aria-hidden="true">
                  {Array.from(profile.name)[0]}
                </span>
                <div>
                  <h2>{profile.name}</h2>
                  <small>
                    {" "}
                    {ui("{count} 个已关联账号", {
                      count: new Set([
                        ...profile.accountIds,
                        ...(view.ui.identityLinks ?? [])
                          .filter((l) => l.playerId === profile.id)
                          .map((l) => l.accountId),
                      ]).size,
                    })}
                  </small>
                </div>
              </div>
              <ProfileTools profile={profile} store={store} />
            </div>
            {profile.accountIds.map((id) => (
              <div className="live-account-binding" key={id}>
                <PlayerHistoryLink
                  target={{
                    platform: id.split(":")[0],
                    puuid: id.slice(id.indexOf(":") + 1),
                    riotId: accountById[id]?.riotId ?? "",
                  }}
                >
                  {playerNames[id] ??
                    accountById[id]?.riotId ??
                    ui("已关联账号（暂未收录对局）")}
                </PlayerHistoryLink>
                <Button
                  appearance="subtle"
                  onClick={() =>
                    store.update((ui) => ({
                      ...ui,
                      players: ui.players.map((p) =>
                        p.id === profile.id
                          ? {
                              ...p,
                              accountIds: p.accountIds.filter((a) => a !== id),
                            }
                          : p,
                      ),
                    }))
                  }
                >
                  {ui("解除关联")}
                </Button>
              </div>
            ))}
            {(view.ui.identityLinks ?? [])
              .filter((l) => l.playerId === profile.id)
              .map((link) => (
                <div className="live-account-binding" key={link.id}>
                  <span>
                    <PlayerHistoryLink
                      target={{
                        platform: link.accountId.split(":")[0],
                        puuid: link.accountId.slice(
                          link.accountId.indexOf(":") + 1,
                        ),
                        riotId: accountById[link.accountId]?.riotId ?? "",
                      }}
                    >
                      {playerNames[link.accountId] ??
                        accountById[link.accountId]?.riotId ??
                        link.accountId}
                    </PlayerHistoryLink>{" "}
                    ·{" "}
                    {link.matchId
                      ? `${t("identityMatch")} · ${link.matchId}`
                      : `${date(link.from!)} — ${date(link.to!)}`}
                  </span>
                  <Button
                    appearance="subtle"
                    onClick={() =>
                      store.update((current) => ({
                        ...current,
                        identityLinks: (current.identityLinks ?? []).filter(
                          (l) => l.id !== link.id,
                        ),
                      }))
                    }
                  >
                    {ui("解除关联")}
                  </Button>
                </div>
              ))}
          </section>
        ))
      ) : (
        <div className="empty-state">
          <People20Regular />
          <h3>{profileQuery ? t("noProfiles") : ui("还没有标记玩家")}</h3>
          {profileQuery ? (
            <Button onClick={() => setProfileQuery("")}>
              {t("clearSearch")}
            </Button>
          ) : (
            <p>{t("associatePlayerHint")}</p>
          )}
        </div>
      )}
    </div>
  );
}
