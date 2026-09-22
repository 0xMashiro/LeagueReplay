import { ui } from "./i18n";
const labels: Record<string, string> = {
  TOP: "上单",
  JUNGLE: "打野",
  MIDDLE: "中单",
  MID: "中单",
  BOTTOM: "下路",
  BOT: "下路",
  UTILITY: "辅助",
  SUPPORT: "辅助",
  NONE: "位置未知",
  Lobby: "大厅",
  Matchmaking: "排队中",
  ReadyCheck: "准备就绪",
  ChampSelect: "英雄选择",
  GameStart: "对局加载中",
  InProgress: "对局进行中",
  WaitingForStats: "等待结算数据",
  PreEndOfGame: "赛后",
  EndOfGame: "赛后",
  None: "准备就绪",
  DRAGON: "巨龙",
  BARON_NASHOR: "纳什男爵",
  RIFTHERALD: "峡谷先锋",
  ATAKHAN: "厄塔汗",
  HORDE: "虚空巢虫",
};
export const gameLabel = (value: string) => ui(labels[value] ?? value);
export const queueLabel = (id: number, fallback: string) =>
  ui(
    (
      {
        400: "匹配",
        420: "单双排位",
        430: "匹配",
        440: "灵活排位",
        450: "极地大乱斗",
        490: "快速模式",
      } as Record<number, string>
    )[id] ?? fallback,
  );
