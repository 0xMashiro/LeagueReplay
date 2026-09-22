import { useEffect, useState, type ReactNode } from "react";
import {
  createDarkTheme,
  createLightTheme,
  FluentProvider,
  type BrandVariants,
  type Theme,
} from "@fluentui/react-components";
import { useThemePreference } from "./theme-preference";
import { useI18n } from "./i18n";

const brand: BrandVariants = {
  10: "#171207",
  20: "#292211",
  30: "#3c311b",
  40: "#504224",
  50: "#65532f",
  60: "#7a653a",
  70: "#907847",
  80: "#a68c55",
  90: "#bca168",
  100: "#ccb47f",
  110: "#d6c194",
  120: "#dfcea9",
  130: "#e8dbbe",
  140: "#efe7d4",
  150: "#f6f1e7",
  160: "#fcfaf5",
};
const common: Partial<Theme> = {
  fontFamilyBase:
    '"Segoe UI Variable", "Segoe UI", "Microsoft YaHei", sans-serif',
  borderRadiusMedium: "6px",
  borderRadiusLarge: "8px",
  borderRadiusXLarge: "12px",
};
const dark: Theme = {
  ...createDarkTheme(brand),
  ...common,
  colorNeutralBackground1: "#1a222d",
  colorNeutralBackground1Hover: "#222e3c",
  colorNeutralBackground1Pressed: "#263342",
  colorNeutralBackground1Selected: "#253342",
  colorNeutralBackground2: "#151c25",
  colorNeutralBackground2Hover: "#1e2936",
  colorNeutralBackground2Pressed: "#293846",
  colorNeutralBackground2Selected: "#26313c",
  colorNeutralBackground3: "#10161e",
  colorNeutralBackground4: "#202b37",
  colorNeutralBackground5: "#273441",
  colorNeutralBackground6: "#2f3e4c",
  colorNeutralForeground1: "#e8edf3",
  colorNeutralForeground2: "#b3bfcd",
  colorNeutralForeground3: "#90a0b3",
  colorNeutralForeground4: "#7c8da0",
  colorNeutralStroke1: "#344354",
  colorNeutralStroke2: "#273442",
  colorNeutralStroke3: "#202b37",
  colorBrandBackground: "#c6aa72",
  colorBrandBackgroundHover: "#d5bc8b",
  colorBrandBackgroundPressed: "#b99c62",
  colorNeutralForegroundOnBrand: "#201b12",
  colorBrandForeground1: "#d5bc8b",
  colorBrandForeground2: "#d5bc8b",
  colorBrandBackground2: "#2c2c27",
  colorBrandBackground2Hover: "#37372d",
  colorBrandBackground2Pressed: "#424132",
  colorBrandStroke1: "#a68c55",
};
const light: Theme = {
  ...createLightTheme(brand),
  ...common,
  colorNeutralBackground1: "#ffffff",
  colorNeutralBackground2: "#f4f6f8",
  colorNeutralBackground3: "#eaf0f4",
  colorBrandBackground: "#786034",
  colorBrandBackgroundHover: "#655029",
  colorBrandBackgroundPressed: "#544222",
  colorNeutralForegroundOnBrand: "#ffffff",
  colorBrandForeground1: "#786034",
  colorBrandForeground2: "#685029",
  colorBrandBackground2: "#f4eddf",
  colorBrandBackground2Hover: "#ebe0c8",
  colorBrandBackground2Pressed: "#dfceaa",
};

export function AppTheme({ children }: { children: ReactNode }) {
  const { language } = useI18n();
  useEffect(() => {
    document.documentElement.lang = language;
  }, [language]);
  const preference = useThemePreference();
  const [systemDark, setSystemDark] = useState(
    matchMedia("(prefers-color-scheme: dark)").matches,
  );
  useEffect(() => {
    const query = matchMedia("(prefers-color-scheme: dark)");
    const change = () => setSystemDark(query.matches);
    query.addEventListener("change", change);
    return () => query.removeEventListener("change", change);
  }, []);
  const theme =
    preference === "system" ? (systemDark ? "dark" : "light") : preference;
  return (
    <FluentProvider
      applyStylesToPortals={false}
      theme={{
        ...(theme === "dark" ? dark : light),
        fontFamilyBase: `"Segoe UI Variable", "Segoe UI", ${language === "ja" ? '"Yu Gothic UI"' : language === "ko" ? '"Malgun Gothic"' : '"Microsoft YaHei"'}, sans-serif`,
      }}
      className="app-theme"
      data-theme={theme}
    >
      {children}
    </FluentProvider>
  );
}
