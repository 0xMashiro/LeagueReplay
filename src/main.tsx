import ReactDOM from "react-dom/client";
import { AppTheme } from "./theme";
import { App } from "./App";
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <AppTheme>
    <App />
  </AppTheme>,
);
