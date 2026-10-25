//! xtask —— Nexus HSB（医院服务总线）开发与发布自动化任务
//!
//! 用法（需在项目根目录执行，`.cargo/config.toml` 已配置别名）：
//!
//! ```bash
//! cargo xtask           # 查看帮助
//! cargo xtask dist      # 编译 release 二进制并打包 dist/ 发布目录（含 tar.gz 与校验和）
//! cargo xtask docker    # 构建 Docker 镜像（--save 导出到 dist/，--push 推送，--platform 多架构）
//! cargo xtask check     # 快速验证：cargo check 整个 workspace
//! ```
//!
//! 说明：本工具与 rust-sso / agent-pro 项目的 xtask 保持同一模式；
//! 生产环境没有 Rust/Cargo 时可直接使用发布包里的 `./hsb-server start`。

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

// ---------------- 输出工具 ----------------

fn use_color() -> bool {
    env::var("NO_COLOR").is_err() && env::var("TERM").map(|t| t != "dumb").unwrap_or(true)
}

fn paint(code: &str, msg: &str) -> String {
    if use_color() {
        format!("\x1b[{code}m{msg}\x1b[0m")
    } else {
        msg.to_string()
    }
}

fn step(idx: usize, total: usize, msg: &str) {
    println!("{} {}", paint("36;1", &format!("[{idx}/{total}]")), msg);
}

fn ok(msg: &str) {
    println!("{} {msg}", paint("32;1", "✅"));
}

fn fail(msg: &str) -> ! {
    eprintln!("{} {msg}", paint("31;1", "❌"));
    std::process::exit(1);
}

// ---------------- 基础工具 ----------------

fn root() -> PathBuf {
    // xtask 位于 <root>/xtask，编译期常量向上一级即项目根
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.parent().expect("无法定位项目根目录").to_path_buf()
}

fn project_version() -> String {
    // 根 Cargo.toml 使用 [workspace.package]，第一个 version 行即工作区版本
    let toml = fs::read_to_string(root().join("Cargo.toml")).expect("读取 Cargo.toml 失败");
    for line in toml.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("version") {
            let rest = rest.trim_start();
            if let Some(v) = rest.strip_prefix('=') {
                let v = v.trim().trim_matches('"');
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
    }
    fail("无法从 Cargo.toml 解析 version 字段");
}

/// 依次执行命令，失败时终止并给出上下文
fn run_cmd(cwd: &Path, program: &str, args: &[&str]) {
    let display = format!("{} {}", program, args.join(" "));
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap_or_else(|e| fail(&format!("无法执行 {program}: {e}（请确认已安装）")));
    if !status.success() {
        fail(&format!("命令执行失败: {display}"));
    }
}

/// 通过 shell 执行（用于管道，如 docker save | gzip）
fn run_shell(cwd: &Path, script: &str) {
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap_or_else(|e| fail(&format!("无法执行 shell: {e}")));
    let status = child.wait().expect("等待 shell 进程失败");
    if !status.success() {
        fail(&format!("shell 命令执行失败: {script}"));
    }
}

fn copy_file(src: &Path, dst: &Path) {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).expect("创建目录失败");
    }
    fs::copy(src, dst).unwrap_or_else(|e| {
        fail(&format!(
            "复制 {} → {} 失败: {e}",
            src.display(),
            dst.display()
        ))
    });
}

fn copy_dir(src: &Path, dst: &Path) -> usize {
    copy_dir_excluding(src, dst, &[])
}

/// 复制目录，跳过排除清单中的文件名
fn copy_dir_excluding(src: &Path, dst: &Path, exclude: &[&str]) -> usize {
    let mut n = 0;
    let entries = fs::read_dir(src)
        .unwrap_or_else(|e| fail(&format!("读取目录 {} 失败: {e}", src.display())));
    for entry in entries {
        let entry = entry.expect("遍历目录失败");
        let name = entry.file_name().to_string_lossy().to_string();
        if exclude.contains(&name.as_str()) {
            continue;
        }
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            n += copy_dir_excluding(&path, &target, exclude);
        } else {
            copy_file(&path, &target);
            n += 1;
        }
    }
    n
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

fn human_size(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|e| fail(&format!("读取目录 {} 失败: {e}", dir.display())));
    for entry in entries {
        let entry = entry.expect("遍历目录失败");
        let p = entry.path();
        if p.is_dir() {
            collect_files(&p, out);
        } else {
            out.push(p);
        }
    }
}

// ---------------- cargo xtask check ----------------

fn cmd_check() {
    let root = root();
    println!("{}", paint("1;36", "🔎 cargo check --workspace"));
    run_cmd(&root, "cargo", &["check", "--workspace"]);
    ok("工作区类型检查通过");
}

// ---------------- cargo xtask dist ----------------

fn cmd_dist(args: &[String]) {
    let no_tar = args.iter().any(|a| a == "--no-tar");
    let version = project_version();
    let root = root();
    let dist_root = root.join("dist");
    let pkg = dist_root.join(format!("nexus-hsb-{version}"));
    let tarball = dist_root.join(format!("nexus-hsb-{version}.tar.gz"));

    println!(
        "{}",
        paint("1;36", &format!("🚀 Nexus HSB v{version} 发布打包 (dist)"))
    );
    println!("  输出目录: {}\n", dist_root.display());

    // 1. 编译 release 二进制
    step(
        1,
        6,
        "编译 release 二进制 (cargo build --release --bin hsb-server) ...",
    );
    run_cmd(
        &root,
        "cargo",
        &["build", "--release", "--bin", "hsb-server"],
    );

    let bin = root.join("target/release/hsb-server");
    if !bin.exists() {
        fail(&format!("未找到编译产物: {}", bin.display()));
    }

    // 2. 组装发布目录
    step(2, 6, "组装发布目录 ...");
    if pkg.exists() {
        fs::remove_dir_all(&pkg).unwrap_or_else(|e| fail(&format!("清理旧目录失败: {e}")));
    }
    fs::create_dir_all(&pkg).unwrap_or_else(|e| fail(&format!("创建目录失败: {e}")));

    copy_file(&bin, &pkg.join("hsb-server"));
    // 保留可执行位（fs::copy 已保留权限位，双保险）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&pkg.join("hsb-server"), fs::Permissions::from_mode(0o755));
    }

    let mut copied = 1usize;
    // 排除开发期的 UI 回归测试配置
    copied += copy_dir_excluding(
        &root.join("config"),
        &pkg.join("config"),
        &["hsb.ui-regression.toml", "hsb.ui-regression-8090.toml"],
    );
    copied += copy_dir(&root.join("deploy"), &pkg.join("deploy"));
    for f in [
        "README.md",
        "Dockerfile",
        "docker-start.sh",
        "start.sh",
        "stop.sh",
    ] {
        let p = root.join(f);
        if p.exists() {
            copy_file(&p, &pkg.join(f));
            copied += 1;
        }
    }

    // 3. 生成部署说明（重点：无 Rust 环境的启动/升级方式）
    step(3, 6, "生成 README-DEPLOY.md ...");
    let deploy_doc = render_deploy_doc(&version);
    fs::write(pkg.join("README-DEPLOY.md"), deploy_doc).expect("写入 README-DEPLOY.md 失败");
    copied += 1;
    ok(&format!("共复制/生成 {copied} 个文件"));

    // 4. 生成 SHA256SUMS
    step(4, 6, "生成校验和 (SHA256SUMS) ...");
    let mut files = Vec::new();
    collect_files(&pkg, &mut files);
    let mut sums = String::new();
    for f in &files {
        let rel = f.strip_prefix(&pkg).unwrap().to_string_lossy().to_string();
        let out = Command::new("sha256sum")
            .arg(f)
            .output()
            .unwrap_or_else(|e| fail(&format!("执行 sha256sum 失败: {e}")));
        if !out.status.success() {
            fail(&format!("sha256sum 计算 {} 失败", f.display()));
        }
        let hash = String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_string();
        sums.push_str(&format!("{hash}  {rel}\n"));
    }
    fs::write(pkg.join("SHA256SUMS"), &sums).expect("写入 SHA256SUMS 失败");
    ok(&format!("共 {} 个文件的校验和", files.len()));

    // 5. 打包 tar.gz
    if no_tar {
        step(5, 6, "跳过 tar.gz 打包 (--no-tar)");
    } else {
        step(5, 6, "打包 tar.gz ...");
        run_cmd(
            &dist_root,
            "tar",
            &[
                "czf",
                tarball.file_name().unwrap().to_string_lossy().as_ref(),
                pkg.file_name().unwrap().to_string_lossy().as_ref(),
            ],
        );
        ok(&format!("{}", tarball.display()));
    }

    // 6. 汇总报告
    step(6, 6, "完成");
    println!("\n{}", paint("1;32", "📦 发布产物清单"));
    println!("  {}  ({})", pkg.display(), human_size(dir_size(&pkg)));
    for f in &files {
        let rel = f.strip_prefix(&pkg).unwrap().display().to_string();
        println!(
            "    {:<40} {}",
            rel,
            human_size(f.metadata().map(|m| m.len()).unwrap_or(0))
        );
    }
    if !no_tar {
        println!(
            "  {}  ({})",
            tarball.display(),
            human_size(tarball.metadata().map(|m| m.len()).unwrap_or(0))
        );
    }
    println!(
        "\n  下一步: 详见 {}/README-DEPLOY.md（生产环境无需 Rust，直接 ./hsb-server start）",
        pkg.display()
    );
}

fn render_deploy_doc(version: &str) -> String {
    format!(
        r#"# Nexus HSB v{version} 生产部署包

本目录为完整的生产发布包。**目标服务器只需要：Linux；启用 PostgreSQL 持久化时另需 PostgreSQL 14+，不需要 Rust/Cargo 工具链。**

## 一、目录说明

| 文件/目录 | 用途 |
|---|---|
| hsb-server | 服务二进制（内嵌 Web 管理台 /ui/ 与管理 API） |
| config/hsb.toml | 配置文件（端口、持久化、消息队列、SSO 等） |
| deploy/k8s | Kubernetes 部署清单 |
| Dockerfile / docker-start.sh | 容器方式部署（可选） |
| start.sh / stop.sh | 裸机启停脚本（PID 与日志管理） |
| SHA256SUMS | 全部文件校验和（sha256sum -c SHA256SUMS 验证） |

## 二、裸机部署（推荐）

```bash
# 1. 校验完整性
sha256sum -c SHA256SUMS

# 2. 按需修改配置（端口 / 数据库 / 消息队列 / SSO）
vim config/hsb.toml

# 3. 检查配置合法性
./hsb-server check --config config/hsb.toml

# 4. 前台启动验证（默认 HTTP 8080 / Admin 8081 / TCP-MLLP 2575 / gRPC 10051）
./hsb-server start --config config/hsb.toml

# 5. 生产运行（后台，带 PID 与日志管理）
./start.sh
```

启动后访问：
- Web 管理台: http://<host>:8080/ui/（消息录入、端点/路由管理、监控）
- 管理 API: http://<host>:8080/api/v1/
- 业务入站: POST http://<host>:8080/api/messages/inbound

## 三、版本升级

```bash
# 用新版本的发布包替换旧目录（保留 config/、data/、logs/）后：
./hsb-server check --config config/hsb.toml   # 校验配置
./stop.sh && ./start.sh                       # 重启生效
```

说明：服务启动时会自动从持久层恢复未完成消息并重放，重启不丢消息。

## 四、Docker 部署（可选）

```bash
# 服务启动（config/data/logs 挂载到宿主机）
./docker-start.sh
```
"#
    )
}

// ---------------- cargo xtask docker ----------------

fn cmd_docker(args: &[String]) {
    let version = project_version();
    let root = root();
    let do_save = args.iter().any(|a| a == "--save");
    let do_push = args.iter().any(|a| a == "--push");
    let platform = args
        .iter()
        .position(|a| a == "--platform")
        .and_then(|i| args.get(i + 1))
        .cloned();

    println!(
        "{}",
        paint("1;36", &format!("🐳 Nexus HSB v{version} Docker 镜像构建"))
    );
    println!("  标签: hsb-server:{version}, hsb-server:latest");
    if platform.is_some() {
        println!("  平台: {}（buildx 多架构）", platform.clone().unwrap());
    }
    println!();

    // 检查 docker
    let has_docker = Command::new("docker")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success());
    if !has_docker {
        fail("docker 命令不可用或守护进程未运行，请先安装/启动 Docker");
    }

    // 构建
    step(1, 3, "构建镜像 ...");
    match platform.as_ref() {
        Some(p) => run_cmd(
            &root,
            "docker",
            &[
                "buildx",
                "build",
                "--platform",
                p,
                "-t",
                &format!("hsb-server:{version}"),
                "-t",
                "hsb-server:latest",
                ".",
                &format!("--push={do_push}"),
            ],
        ),
        None => run_cmd(
            &root,
            "docker",
            &[
                "build",
                "-t",
                &format!("hsb-server:{version}"),
                "-t",
                "hsb-server:latest",
                ".",
            ],
        ),
    }
    ok(&format!("hsb-server:{version} / hsb-server:latest"));

    // 导出
    if do_save {
        let dist = root.join("dist");
        fs::create_dir_all(&dist).expect("创建 dist 失败");
        let out = dist.join(format!("hsb-server-{version}-docker.tar.gz"));
        step(2, 3, &format!("导出镜像到 {} ...", out.display()));
        run_shell(
            &root,
            &format!(
                "docker save hsb-server:{version} | gzip > {}",
                out.display()
            ),
        );
        ok(&format!(
            "{} ({})",
            out.display(),
            human_size(out.metadata().map(|m| m.len()).unwrap_or(0))
        ));
    } else {
        step(2, 3, "跳过导出（如需导出 tar.gz 请加 --save）");
    }

    // 推送
    if do_push {
        if platform.is_none() {
            step(3, 3, "推送镜像 ...");
            run_cmd(&root, "docker", &["push", &format!("hsb-server:{version}")]);
            run_cmd(&root, "docker", &["push", "hsb-server:latest"]);
            ok("已推送 hsb-server:version 与 :latest");
        } else {
            step(3, 3, "推送已由 buildx --push 完成");
        }
    } else {
        step(3, 3, "完成（未推送；如需推送请加 --push）");
    }
}

// ---------------- main ----------------

fn help() {
    println!("{}", paint("1;36", "Nexus HSB 自动化任务 (xtask)"));
    println!(
        "
用法: cargo xtask <命令> [选项]

命令:
  dist        编译 release 二进制并组装 dist/ 发布包（含配置、部署清单、启停脚本、
              校验和与 tar.gz），并输出产物清单
                --no-tar         只组装目录，不生成 tar.gz
  docker      构建 Docker 镜像 hsb-server:<version> 与 :latest
                --save           导出镜像 tar.gz 到 dist/
                --push           构建后推送
                --platform <p>   buildx 多架构（如 linux/amd64,linux/arm64）
  check       快速验证（cargo check --workspace）

说明:
  * 发布包自带 Web 管理台（/ui/）与管理 API（/api/v1），生产环境无需 Rust 工具链，
    直接运行 ./hsb-server start。
  * dist 产物输出到项目根的 dist/ 目录，包含 SHA256SUMS 校验和。
"
    );
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("dist") => cmd_dist(&args[1..]),
        Some("docker") => cmd_docker(&args[1..]),
        Some("check") => cmd_check(),
        Some("help") | Some("--help") | Some("-h") | None => help(),
        other => {
            eprintln!("{} 未知命令: {other:?}\n", paint("31;1", "❌"));
            help();
            std::process::exit(2);
        }
    }
}
