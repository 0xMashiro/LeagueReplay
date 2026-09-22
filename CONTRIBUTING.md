# 参与开发

需要 Windows、Node.js 22.12+、Rust stable（`x86_64-pc-windows-msvc`）、Visual Studio C++ Build Tools 和 WebView2。

```sh
npm ci
npm run tauri dev
```

开发端口为 1420。构建 Windows 安装包：

```sh
npm run package:windows
```

安装包输出到 `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`。

## 检查与贡献

- 开发命令以 [package.json](package.json) 为准，完整检查顺序见 [Windows CI](.github/workflows/windows.yml)。布局测试默认使用 Microsoft Edge。
- Rust 导出类型变化后运行 `npm run types:generate`；不要手改生成文件。提交前运行 `npm run types:check`，并先于可能重新导出类型的 Rust 测试执行。
- 真实客户端测试默认忽略。按测试的 `ignore` 注解和环境变量要求单独运行，勿批量启用；下载和播放测试可能写文件或启动游戏。[跨区只读测试](src-tauri/src/library/mod.rs)、[录像测试](src-tauri/src/replays/live_tests.rs)。
- 提交说明写清问题、变更和验证结果。界面改动附截图，翻译修改同步四种语言。
- 不提交真实账号凭据、个人归档、录像或测试产物。保留相关许可与第三方声明。

安全问题按 [SECURITY.md](SECURITY.md) 私下报告。
