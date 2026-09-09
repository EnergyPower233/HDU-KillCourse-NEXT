# 维护指南

## 从哪里开始读

先读 `src/types.ts` 与 `server/src/model.rs`，了解课程、任务、清单、设置和运行事件。字段 `jxbmc`、`jxb_id`、`kch_id` 沿用教务协议，分别代表教学班编号、教学班内部编号、课程内部编号；不要因名称相似而互换。

前端入口是 `src/App.tsx`，负责连接共享状态、页面与弹窗。课程、任务和设置的布局在 `src/pages/`；日志界面在 `src/ActivityLog.tsx`。页面说明集中在 `components/PageHeader.tsx`。

`hooks/useWorkspace.ts` 读取本机数据并轮询状态；`useLogin.ts` 管理账号和扫码流程；`useTaskLists.ts` 管理清单编辑、导入和导出；`useTheme.ts` 同步外观。`domain/` 放不依赖 React 或网络的业务辅助函数，`types.ts` 只放数据类型。

后端入口 `server/src/lib.rs` 只负责启动与关闭服务，以及启动时的自动登录。`api/mod.rs` 列出全部本机路由，请求处理按课程、登录、配置、任务和日志划分文件。`client/` 与学校通信，`scheduler.rs` 决定任务如何执行，`storage.rs` 处理本地配置与课程缓存，`run_log.rs` 处理运行日志。

## 常见修改入口

| 要修改的内容                   | 入口                                                                        |
| ------------------------------ | --------------------------------------------------------------------------- |
| 课程搜索与列表                 | `pages/CoursesPage.tsx`、`domain/courses.ts`                                |
| 清单操作与导入导出             | `pages/TasksPage.tsx`、`hooks/useTaskLists.ts`、`server/src/task_import.rs` |
| 登录界面与轮询                 | `components/LoginDialog.tsx`、`hooks/useLogin.ts`                           |
| 学校登录、课程下载、选退课表单 | `server/src/client/auth.rs`、`courses.rs`、`enrollment.rs`                  |
| 执行顺序、并发与停止           | `server/src/scheduler.rs`                                                   |
| 设置文件位置、旧目录迁移       | `server/src/storage.rs`                                                     |
| 日志保存、历史查询与展示       | `server/src/run_log.rs`、`api/activity.rs`、`src/ActivityLog.tsx`           |

## 需要维持的规则

- 页面不能直接拼学校请求；前端通过 `bridge.ts` 访问本机 API，学校会话由 Rust 持有。
- 清单 JSON 解析集中在 `task_import.rs`，课程缓存解析集中在 `model.rs`；不要另写一个前端版本来维护相同的导入规则。
- 清单导入先整体验证再追加，失败保留原编辑内容；只导入与保存都不能自动执行任务。
- 任务执行必须经过用户确认。余量查询可以并发，提交串行；已发出的选退课请求无法撤回。
- `Success`、`Rejected`、`Unknown` 是三种不同结果；未知结果不能作为失败自动重试。日志保存课程身份和操作类型，不靠当前课程缓存解释历史。
- `state.rs` 的锁保护共享状态，异步学校请求之前先释放锁。当前日志落盘仍在发布事件时同步完成，大规模日志的性能改进应单独验证。
- 配置是本地 JSON，凭据仍是明文持久化；存储模块拆分没有改变这个事实。服务退出、登录竞态等已知限制保留在 Protocol Review，不视为此次整理已解决。

## 修改后如何验证

```sh
npm ci
npm run format
npm run format:check
npm run build
npm test
npm run server:test
cargo clippy --locked --manifest-path server/Cargo.toml --lib -- -D warnings
```

前端测试用模拟 API 验证页面切换、清单导入、保存与任务确认；Rust 测试覆盖清单规则、学校响应、课程分页和日志。运行 Rust 静态资源测试前先构建前端。测试不需要学校凭据。

前端启用 TypeScript 未使用变量与参数检查，格式由 Prettier 统一；Rust 使用 rustfmt。新增注释解释约束、原因或协议来源，避免描述已经废弃的架构，或写“始终成功”“不会被检测”等实现无法保证的结论。
