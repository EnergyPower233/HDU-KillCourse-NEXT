# 自动构建与 Release

工作流：`.github/workflows/release.yml`。不需要填写个人访问令牌，发布任务使用 GitHub 自动提供的 GITHUB_TOKEN；仓库或组织须允许工作流写入内容。

## 第一次使用

先把工作流、本文和 README 的追加说明提交到仓库的 main 分支。

```sh
git add .github/workflows/release.yml docs/releasing.md README.md .gitignore
git commit -m "Add multi-platform release workflow"
git push origin main
```

先在 GitHub 的 Actions → Build and Release → Run workflow 试跑。手动运行只生成可下载的 Artifacts，不创建 Release。六个平台都成功后，再创建标签。

## 发布 v0.3.0

确认 package.json、package-lock.json 与 server/Cargo.toml 的版本都是 0.3.0，且对应修改已提交。若已有 v0.3.0 标签，不要删除或强推它；使用递增版本并同步更新锁文件。

```sh
git tag v0.3.0
git push origin v0.3.0
```

推送标签触发六个平台的构建：Windows x64/ARM64、macOS Intel/Apple Silicon、Linux x64/ARM64。每个平台先测试并构建界面，再测试 Rust 并编译内嵌界面的正式程序。

全部通过后，在仓库 Releases 中生成草稿，默认勾选 Pre-release。检查附件和更新说明，点击 Publish release。这里使用草稿是为了给附件和版本说明留出最后检查步骤。

## 下载内容

- Windows：ZIP，包含 hdu-course-server.exe。
- macOS / Linux：tar.gz，保留 hdu-course-server 的可执行权限。
- 每份包都包含完整 README、LICENSE、CONTRIBUTING、docs 和 examples，不包含个人 data 或编译缓存。
- SHA256SUMS.txt：六份压缩包的 SHA-256 校验值。

macOS 产物未签名或公证；Windows 产物未做代码签名。Linux 使用 Ubuntu 22.04 构建，不能保证比该构建环境更旧的系统可运行。六平台配置不等于六平台已经通过：首次 Actions 实际结果才是依据。

## 失败或重跑

- 查看 Actions 中失败步骤。任一平台失败时，不创建 Release；其他成功平台的 Artifacts 仍可下载，保留 14 天。
- 修正源码后用新的版本标签发布。仅环境故障可在同一提交上 Re-run failed jobs。
- 若草稿创建或上传中断，可以重跑发布任务；它会补充/覆盖同名草稿附件。已发布 Release 不会被自动覆盖。
- 标签必须是 v 加源码版本，例如 v0.3.0；版本检查不通过时，先同步 package.json、package-lock.json、Cargo.toml 与 Cargo.lock。
- 如果构建完成但创建 Release 报 403，检查 Settings → Actions → General 的工作流权限，以及组织是否限制 contents: write。

官方参考：

- [GitHub 托管运行器](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
- [Release 管理](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)
- [GitHub CLI release create](https://cli.github.com/manual/gh_release_create)
