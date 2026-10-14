# FileHive - Rust Edition (`FileHive`)

基于 **Axum + SQLx + Apache OpenDAL + Tokio** 的高性能文件与文本快递柜实现。本项目基于开源项目 [FileCodeBox (vastsa/FileCodeBox)](https://github.com/vastsa/FileCodeBox) 二次开发，提供 100% 数据兼容与 API 兼容的原位平替。

---

## 架构概览

本项目采用 **Cargo Workspace** 多 Crate 分层架构设计，实现了模块化、高解耦与高内聚：

```
FileHive/
├── Cargo.toml                  # Workspace 顶级清单
├── Dockerfile                  # 轻量容器多阶段构建
├── frontend/                   # Vue 3 前端源码 (FileHiveFrontend)
├── themes/                     # 前端编译静态产物 (2024 主题)
└── crates/
    ├── filehive-core/          # 基础类型、统一定义 (APIResponse, ErrorResponse, AppError, 版本常量)
    ├── filehive-storage/       # 基于 Apache OpenDAL 的统一多存储驱动 (Local, S3, WebDAV, OneDrive)
    ├── filehive-db/            # 多数据库连接池 (SQLite / PostgreSQL / MySQL) 与全套 CRUD
    ├── filehive-security/      # scrypt 密码哈希兼容、JWT 鉴权、出站 SSRF 防御
    ├── filehive-config/        # 基于 ArcSwap 的零锁配置热更新、内存滑动窗口 IP 限流、配额管理
    └── filehive-server/        # Axum 业务服务 (分发 /share, /chunk, /presign, /admin, /mcp, /setup)
```

---

## 核心特性与技术亮点

1. **原位平替与数据 100% 兼容**：
   - 数据表结构完全兼容原版 SQLite `FileHive.db` (`filecodes`, `uploadchunk`, `keyvalue`, `presignuploadsession`, `storagereservation`)。
   - 密码验证严格复现 Python `scrypt$16384$8$1${salt}${hash}` 格式，存量实例更换为 Rust 二进制后可无缝平滑升级，无需重置密码。
2. **多数据库原生支持 (SQLite / PostgreSQL / MySQL)**：
   - 根据 `DATABASE_URL` 环境变量自动识别底层数据库类型并完成连接池创建与自建表迁移：
     - SQLite: `sqlite:data/FileHive.db` 或普通文件路径 `data/FileHive.db`
     - PostgreSQL: `postgres://user:password@localhost:5432/FileHive`
     - MySQL / MariaDB: `mysql://user:password@localhost:3306/FileHive`
3. **零锁配置管理 (`arc_swap`)**：
   - 读请求完全无锁，管理员修改配置时通过 `ArcSwap` 原子替换全局配置指针，避免读写锁争用。
4. **原生跨存储支持 (`Apache OpenDAL`)**：
   - 彻底废除多套孤立驱动，直接依托 Rust 生态最强大的存储抽象引擎，无缝覆盖本地存储、AWS S3、MinIO、WebDAV 与 OneDrive。
5. **极低内存占用与单二进制分发**：
   - 内存占用从 Python 运行时的 150MB~200MB 下降至 10MB~20MB。
   - 前端已内置并支持直接在 `frontend/` 下热重载开发或编译部署。

---

## 构建与运行

### 1. 本地编译与测试
```bash
# 进入工作空间
cd FileHive

# 运行全套单元测试
cargo test --workspace

# 编译服务器二进制
cargo build --release -p filehive-server
```

### 2. 前端构建 (可选自定义)
```bash
cd frontend
pnpm install
pnpm run build-only # 自动打包至 themes/2024
```

### 3. 启动服务

#### 模式 A: 使用默认 SQLite
```bash
export PORT=12345
export HOST="0.0.0.0"
export DATABASE_URL="data/FileHive.db"
./target/release/filehive
```

#### 模式 B: 使用 PostgreSQL
```bash
export DATABASE_URL="postgres://postgres:password@localhost:5432/FileHive"
./target/release/filehive
```

#### 模式 C: 使用 MySQL
```bash
export DATABASE_URL="mysql://root:password@localhost:3306/FileHive"
./target/release/filehive
```

---

## 接口覆盖度清单

| 路由前缀 | 主要功能 | 状态 |
| :--- | :--- | :--- |
| `/share/text/` | 文本分享上传 | ✅ 已实现 |
| `/share/file/` | 文件分发上传 (Multipart) | ✅ 已实现 |
| `/share/select/` | 取件码解析与取件 | ✅ 已实现 |
| `/share/download` | 文件下载与验签 | ✅ 已实现 |
| `/share/metadata/` | 元数据只读查询 | ✅ 已实现 |
| `/chunk/*` | 分片上传初始化、上传分片、合并与取消 | ✅ 已实现 |
| `/presign/*` | S3 预签名直传与代理上传初始化 | ✅ 已实现 |
| `/admin/*` | 管理后台鉴权、仪表盘、文件列表、批量删除、配置热更、本地路径挂载 | ✅ 已实现 |
| `/mcp` | Model Context Protocol AI 智能体集成接口 | ✅ 已实现 |
| `/setup` | 首次部署系统初始化向导 | ✅ 已实现 |
| `/` & `/assets/*` | 前端单页应用 (SPA) 动态注入与静态托管 | ✅ 已实现 |
| `/health` | 服务健康检查 | ✅ 已实现 |

---

## 开源协议与致谢

- 本项目基于 [FileCodeBox](https://github.com/vastsa/FileCodeBox) 进行二次开发与 Rust 高性能重构。
- 感谢原作者 [vastsa](https://github.com/vastsa) 及所有贡献者的开源工作！
- 本项目遵循 [LGPL-3.0 License](LICENSE) 开源协议。

