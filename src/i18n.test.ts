import { afterEach, describe, expect, it } from "vitest";
import { explainError, setLanguage, translate } from "./i18n";

afterEach(() => setLanguage("zh-CN"));

describe("application error presentation", () => {
  it("translates stable codes without parsing backend prose", () => {
    setLanguage("en");
    expect(explainError("ui.invalidNote")).toBe(
      "Invalid note content or timestamp",
    );
    expect(explainError("game.notFinished")).toBe(
      "Match results are not ready",
    );
    expect(explainError("library.storageError")).toBe(
      translate("storageError"),
    );
    setLanguage("ja");
    expect(explainError("ui.invalidTheme")).toBe("テーマが無効です");
  });

  it("does not display unexpected backend details or treat prototype names as codes", () => {
    for (const language of ["zh-CN", "en", "ja", "ko"] as const) {
      setLanguage(language);
      for (const reason of [
        "数据库内部详情",
        "unknown.error",
        "toString",
        "__proto__",
        { detail: "private path" },
      ]) {
        expect(explainError(reason)).toBe(translate("generalError"));
      }
    }
  });
});
