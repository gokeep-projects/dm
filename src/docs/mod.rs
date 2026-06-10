use chrono::Local;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;

const DEFAULT_DOC_ID: &str = "first-use-quick-start";
const DEFAULT_DOC_TITLE: &str = "第一次使用，教你如何快速使用";
const DEFAULT_DOC_CATEGORY: &str = "系统文档";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocMeta {
    pub id: String,
    pub title: String,
    pub category: String,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doc {
    pub meta: DocMeta,
    pub content: String,
}

fn docs_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".dm").join("docs")
}

fn dirs_path() -> PathBuf {
    docs_dir().join(".dirs.json")
}

pub fn ensure_docs_dir() {
    let _ = std::fs::create_dir_all(docs_dir());
}

pub fn ensure_default_docs() {
    ensure_docs_dir();
    if let Some(doc) = get_doc(DEFAULT_DOC_ID) {
        let should_repair = doc.content.contains("编辑时左侧写 Markdown 源码")
            || !doc.content.contains("## 快速上手路径");
        if should_repair {
            let _ = update_doc(
                DEFAULT_DOC_ID,
                Some(DEFAULT_DOC_TITLE),
                Some(DEFAULT_DOC_CATEGORY),
                None,
                Some(default_quick_start_content()),
            );
        }
        return;
    }
    let _ = create_doc(
        DEFAULT_DOC_ID,
        DEFAULT_DOC_TITLE,
        DEFAULT_DOC_CATEGORY,
        default_quick_start_content(),
    );
}

fn default_quick_start_content() -> &'static str {
    r#"DM 是面向现场运维和应急排障的离线优先控制台。它把系统体检、维护脚本、服务管理、流量分析、Java 运行时诊断和维护文档集中在一个本地应用里，适合服务器、内网、容器宿主机和无法稳定访问外网的环境。

## 快速上手路径

1. 打开首页仪表盘，先确认主机、CPU、内存、磁盘、负载、网络流量和 TOP 进程是否正常。
2. 进入系统设置，检查 Web 端口、脚本目录、数据目录、常规检查连接配置和 Redis/数据库等基础连接。
3. 进入常规检查，点击一键体检，观察实时日志和告警结果。重复告警会自动合并，详情里保留证据和处理建议。
4. 进入维护脚本，上传或选择脚本执行。脚本支持 shell、Python、Perl 等可执行文件，执行前确认参数，执行后查看实时输出和历史记录。
5. 进入流量分析，选择网卡或导入 PCAP，查看 HTTP/TCP/UDP 明文解析、请求方法、请求体、响应体和导出的原始包。
6. 进入堆栈分析，选择 Java 进程后先做快速分析；需要持续观察时开启实时跟踪，再导出 HPROF、PDF 报告或原始数据。

## 维护文档怎么写

这个页面不是只能写 Markdown。你可以像写一份现场记录一样直接输入文字，也可以粘贴命令、日志、截图和附件。系统会自动保存，适合记录以下信息：

- 现象：用户影响、发生时间、错误提示、相关服务。
- 判断：关键指标、日志片段、流量证据、堆栈证据。
- 操作：执行过的命令、脚本参数、变更内容、回滚方式。
- 结果：恢复时间、验证方式、残留风险、后续优化项。

## 推荐记录模板

### 背景

记录故障开始时间、影响范围、相关业务、告警来源和当前负责人。

### 证据

粘贴关键日志、截图、请求片段、堆栈热点、线程状态、慢调用或流量明文。

### 操作

写清楚每一步命令、脚本、参数、目标机器和预期结果。重要命令建议使用代码块：

```bash
dm serve -d
dm scripts run <script-id> -- --env prod
```

### 验证

记录服务健康检查、接口验证、指标回落、用户侧确认和是否需要继续观察。

## 常用入口

- 后台启动：`dm serve -d`
- Web 控制台：默认从本机服务端口进入
- CLI 帮助：`dm --help`
- 维护脚本：先上传脚本，再执行和复盘
- Java 排障：先快速分析，再实时跟踪，最后导出证据

把这篇默认文档保留下来，后续可以把团队标准流程、常用检查项、回滚模板和典型故障案例继续补充到维护文档里。
"#
}

fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect()
}

pub fn list_docs(category: Option<&str>) -> Vec<DocMeta> {
    ensure_default_docs();
    let dir = docs_dir();
    if !dir.exists() {
        return Vec::new();
    }
    let mut docs = Vec::new();
    for entry in std::fs::read_dir(&dir).into_iter().flatten() {
        let Ok(e) = entry else {
            continue;
        };
        let p = e.path();
        if p.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(doc) = load_doc_meta(&p) {
            if let Some(cat) = category {
                if doc.category != cat {
                    continue;
                }
            }
            docs.push(doc);
        }
    }
    docs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    docs
}

fn read_stored_doc_dirs() -> Vec<String> {
    let path = dirs_path();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    serde_json::from_str::<Vec<String>>(&raw)
        .unwrap_or_default()
        .into_iter()
        .map(|dir| dir.trim().to_string())
        .filter(|dir| !dir.is_empty())
        .collect()
}

fn write_stored_doc_dirs(dirs: &[String]) -> Result<(), String> {
    ensure_docs_dir();
    let mut set = BTreeSet::new();
    for dir in dirs {
        let clean = dir.trim();
        if !clean.is_empty() {
            set.insert(clean.to_string());
        }
    }
    let values: Vec<String> = set.into_iter().collect();
    let raw = serde_json::to_string_pretty(&values).map_err(|e| e.to_string())?;
    std::fs::write(dirs_path(), raw).map_err(|e| e.to_string())
}

fn collect_doc_dirs() -> Vec<String> {
    let mut set = BTreeSet::new();
    set.insert(DEFAULT_DOC_CATEGORY.to_string());
    set.insert("通用".to_string());
    for dir in read_stored_doc_dirs() {
        set.insert(dir);
    }
    let dir = docs_dir();
    if dir.exists() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten() {
            let Ok(e) = entry else {
                continue;
            };
            let p = e.path();
            if p.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            if let Some(doc) = load_doc_meta(&p) {
                if !doc.category.trim().is_empty() {
                    set.insert(doc.category);
                }
            }
        }
    }
    set.into_iter().collect()
}

pub fn list_doc_dirs() -> Vec<String> {
    ensure_default_docs();
    collect_doc_dirs()
}

pub fn create_doc_dir(name: &str) -> Result<Vec<String>, String> {
    let clean = name.trim();
    if clean.is_empty() {
        return Err("目录名称不能为空".to_string());
    }
    if clean.len() > 80 {
        return Err("目录名称过长".to_string());
    }
    if clean
        .chars()
        .any(|c| matches!(c, '/' | '\\' | '\0' | '\r' | '\n'))
    {
        return Err("目录名称不能包含路径分隔符或换行".to_string());
    }
    ensure_docs_dir();
    let mut dirs = read_stored_doc_dirs();
    if !dirs.iter().any(|dir| dir == clean) {
        dirs.push(clean.to_string());
        write_stored_doc_dirs(&dirs)?;
    }
    Ok(collect_doc_dirs())
}

fn load_doc_meta(path: &std::path::Path) -> Option<DocMeta> {
    let content = std::fs::read_to_string(path).ok()?;
    let id = path.file_stem()?.to_string_lossy().to_string();
    let meta = std::fs::metadata(path).ok()?;
    let mut title = id.clone();
    let mut category = "未分类".to_string();
    let mut tags = Vec::new();
    let mut created_at = String::new();
    let mut updated_at = String::new();

    for line in content.lines() {
        if line.starts_with("# ") && title == id {
            title = line[2..].trim().to_string();
        }
        if line.starts_with("<!-- category:") {
            category = line
                .trim_start_matches("<!-- category:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
        }
        if line.starts_with("<!-- tags:") {
            let t = line
                .trim_start_matches("<!-- tags:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
            tags = t
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if line.starts_with("<!-- created:") {
            created_at = line
                .trim_start_matches("<!-- created:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
        }
        if line.starts_with("<!-- updated:") {
            updated_at = line
                .trim_start_matches("<!-- updated:")
                .trim_end_matches("-->")
                .trim()
                .to_string();
        }
    }

    if created_at.is_empty() {
        created_at = meta
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|_d| Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
    }
    if updated_at.is_empty() {
        updated_at = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|_d| Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default();
    }

    Some(DocMeta {
        id,
        title,
        category,
        tags,
        created_at,
        updated_at,
        size_bytes: meta.len(),
    })
}

pub fn get_doc(id: &str) -> Option<Doc> {
    let safe_id = sanitize_id(id);
    let path = docs_dir().join(format!("{}.md", safe_id));
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    let meta = load_doc_meta(&path)?;
    Some(Doc { meta, content })
}

pub fn create_doc(id: &str, title: &str, category: &str, content: &str) -> Result<DocMeta, String> {
    let safe_id = sanitize_id(id);
    if safe_id.is_empty() {
        return Err("无效的文档 ID".to_string());
    }
    ensure_docs_dir();
    let path = docs_dir().join(format!("{}.md", safe_id));
    if path.exists() {
        return Err("文档已存在".to_string());
    }

    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let full_content = format!("<!-- category:{} -->\n<!-- tags: -->\n<!-- created:{} -->\n<!-- updated:{} -->\n\n# {}\n\n{}", category, now, now, title, content);
    std::fs::write(&path, &full_content).map_err(|e| e.to_string())?;
    let _ = create_doc_dir(category);
    load_doc_meta(&path).ok_or_else(|| "创建失败".to_string())
}

#[allow(dead_code)]
pub fn update_doc(
    id: &str,
    title: Option<&str>,
    category: Option<&str>,
    tags: Option<&str>,
    content: Option<&str>,
) -> Result<DocMeta, String> {
    let safe_id = sanitize_id(id);
    let path = docs_dir().join(format!("{}.md", safe_id));
    if !path.exists() {
        return Err("文档不存在".to_string());
    }

    let existing = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    let mut lines: Vec<String> = existing.lines().map(|l| l.to_string()).collect();

    if let Some(t) = title {
        for line in lines.iter_mut() {
            if line.starts_with("# ") {
                *line = format!("# {}", t);
                break;
            }
        }
    }

    if let Some(c) = category {
        for line in lines.iter_mut() {
            if line.starts_with("<!-- category:") {
                *line = format!("<!-- category:{} -->", c);
                break;
            }
        }
    }

    if let Some(t) = tags {
        for line in lines.iter_mut() {
            if line.starts_with("<!-- tags:") {
                *line = format!("<!-- tags:{} -->", t);
                break;
            }
        }
    }

    for line in lines.iter_mut() {
        if line.starts_with("<!-- updated:") {
            *line = format!("<!-- updated:{} -->", now);
            break;
        }
    }

    if let Some(c) = content {
        let title_line = lines
            .iter()
            .position(|line| line.starts_with("# "))
            .unwrap_or_else(|| {
                lines.push(format!("# {}", title.unwrap_or(&safe_id)));
                lines.len() - 1
            });
        let mut result = lines[..=title_line].to_vec();
        result.push(String::new());
        let body = strip_doc_envelope(c);
        result.extend(body.lines().map(|line| line.to_string()));
        lines = result;
    }

    let has_title = lines.iter().any(|line| line.starts_with("# "));
    if !has_title {
        lines.push(String::new());
        lines.push(format!("# {}", title.unwrap_or(&safe_id)));
    }

    let final_content = lines.join("\n");
    std::fs::write(&path, &final_content).map_err(|e| e.to_string())?;
    if let Some(c) = category {
        let _ = create_doc_dir(c);
    }
    load_doc_meta(&path).ok_or_else(|| "更新失败".to_string())
}

fn strip_doc_envelope(content: &str) -> String {
    let mut seen_title = false;
    let mut body = Vec::new();
    for line in content.lines() {
        if line.starts_with("<!-- ") {
            continue;
        }
        if !seen_title && line.starts_with("# ") {
            seen_title = true;
            continue;
        }
        if seen_title || !line.trim().is_empty() {
            body.push(line);
            seen_title = true;
        }
    }
    body.join("\n").trim().to_string()
}

pub fn delete_doc(id: &str) -> Result<(), String> {
    let safe_id = sanitize_id(id);
    if safe_id == DEFAULT_DOC_ID {
        return Err("默认系统文档不能删除".to_string());
    }
    let path = docs_dir().join(format!("{}.md", safe_id));
    if !path.exists() {
        return Err("文档不存在".to_string());
    }
    std::fs::remove_file(&path).map_err(|e| e.to_string())
}

pub fn upload_file(doc_id: &str, filename: &str, data: &[u8]) -> Result<String, String> {
    let safe_id = sanitize_id(doc_id);
    if safe_id.is_empty() {
        return Err("无效的文档 ID".to_string());
    }
    ensure_docs_dir();
    let attach_dir = docs_dir().join(format!("{}_attachments", safe_id));
    std::fs::create_dir_all(&attach_dir).map_err(|e| e.to_string())?;
    let safe_name: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    if safe_name.is_empty() {
        return Err("无效的文件名".to_string());
    }
    let path = attach_dir.join(&safe_name);
    std::fs::write(&path, data).map_err(|e| e.to_string())?;
    Ok(format!("{}/{}", safe_id, safe_name))
}

pub fn download_file(doc_id: &str, filename: &str) -> Result<Vec<u8>, String> {
    let safe_id = sanitize_id(doc_id);
    let safe_name: String = filename
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '.' || *c == '-' || *c == '_')
        .collect();
    let path = docs_dir()
        .join(format!("{}_attachments", safe_id))
        .join(&safe_name);
    if !path.exists() {
        return Err("文件不存在".to_string());
    }
    std::fs::read(&path).map_err(|e| e.to_string())
}

pub fn list_attachments(doc_id: &str) -> Vec<String> {
    let safe_id = sanitize_id(doc_id);
    let attach_dir = docs_dir().join(format!("{}_attachments", safe_id));
    if !attach_dir.exists() {
        return Vec::new();
    }
    std::fs::read_dir(&attach_dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}

pub fn list_dir_files(dir_path: &str) -> Result<Vec<serde_json::Value>, String> {
    let path = std::path::PathBuf::from(dir_path);
    if !path.exists() {
        return Err("目录不存在".to_string());
    }
    if !path.is_dir() {
        return Err("不是目录".to_string());
    }

    let mut files = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())? {
        let Ok(e) = entry else { continue };
        let p = e.path();
        let meta = e.metadata().map_err(|e| e.to_string())?;
        files.push(serde_json::json!({
            "name": p.file_name().unwrap_or_default().to_string_lossy(),
            "path": p.display().to_string(),
            "is_dir": p.is_dir(),
            "size": meta.len(),
            "modified": meta.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0),
        }));
    }
    files.sort_by(|a, b| {
        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
        b_dir.cmp(&a_dir).then_with(|| {
            a["name"]
                .as_str()
                .unwrap_or("")
                .cmp(b["name"].as_str().unwrap_or(""))
        })
    });
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn isolated_home(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("dm-docs-test-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HOME", &dir);
        dir
    }

    #[test]
    fn update_doc_replaces_body_without_duplicating_title() {
        let _guard = TEST_LOCK.lock().unwrap();
        let home = isolated_home("update-doc");
        let id = format!("unit-doc-{}", std::process::id());
        let _ = super::delete_doc(&id);
        super::create_doc(&id, "旧标题", "测试", "旧正文").unwrap();

        super::update_doc(
            &id,
            Some("新标题"),
            Some("测试"),
            None,
            Some("## 步骤\n\n- 执行命令"),
        )
        .unwrap();
        let doc = super::get_doc(&id).unwrap();

        assert_eq!(doc.meta.title, "新标题");
        assert!(doc.content.contains("# 新标题"));
        assert!(doc.content.contains("## 步骤"));
        assert_eq!(doc.content.matches("# 新标题").count(), 1);
        assert!(!doc.content.contains("旧正文"));
        let _ = super::delete_doc(&id);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn default_quick_start_doc_cannot_be_deleted() {
        let _guard = TEST_LOCK.lock().unwrap();
        let home = isolated_home("default-doc");

        super::ensure_default_docs();
        let result = super::delete_doc("first-use-quick-start");

        assert!(result.is_err());
        let doc =
            super::get_doc("first-use-quick-start").expect("default quick-start doc should remain");
        assert_eq!(doc.meta.title, "第一次使用，教你如何快速使用");
        assert!(doc.content.contains("## 快速上手路径"));

        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn doc_dirs_persist_without_docs() {
        let _guard = TEST_LOCK.lock().unwrap();
        let home = isolated_home("doc-dirs");

        super::create_doc_dir("数据库").unwrap();
        let dirs = super::list_doc_dirs();

        assert!(dirs.iter().any(|dir| dir == "数据库"));
        assert!(dirs.iter().any(|dir| dir == "系统文档"));
        assert!(super::docs_dir().join(".dirs.json").exists());

        let _ = std::fs::remove_dir_all(home);
    }
}
