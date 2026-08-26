# 贡献指南

欢迎对本项目提出问题、报告缺陷或提交改进。

## 反馈渠道

- **问题与缺陷**：在 GitHub Issues 提交，请附上宿主软件与版本（通达信版本号 / MT4 内部版本号）、DLL 文件名与位数、复现步骤、以及必要的日志或截图。
- **使用与讨论**：在 GitHub Discussions 提问或交流。

## 提交代码

1. 先开一个 Issue 说明改动意图，避免重复劳动。
2. Fork 仓库，从 `main` 建立分支，一个分支只做一件事。
3. 提交信息使用中文、描述变更本身，例如 `docs: 补充 MT4 安装说明`、`fix(tdx): 修正 DLL 绑定回读判断`。
4. 提交前在本地跑通对应层的测试：

   ```powershell
   cd chanlun; cargo test
   cd tdx_dll; cargo test
   cd mt4_dll; cargo test
   ```

5. 发起 Pull Request，说明改动内容、验证方式与影响范围。

## 范围说明

- 算法库 `chanlun/` 是结构计算的唯一来源，改动需附回归测试与实测对照。
- 通达信公式文件为 GBK（GB2312）编码，保存为 UTF-8 会导致导入乱码。
- MT4 DLL 需 32 位目标 `i686-pc-windows-msvc`；通达信 DLL 为 64 位 `x86_64-pc-windows-msvc`。
- 请一并核对 [README.md](README.md)、[傻瓜式使用说明书.md](傻瓜式使用说明书.md) 与发布包中的说明是否需要同步。

## 许可证

本项目采用 [MIT 协议](LICENSE)。提交贡献即表示同意按同一协议共享。
