use super::*;

const BUSINESS_CHECKS: &[&str] = &[
    "system",
    "resource",
    "service",
    "network",
    "security",
    "middleware",
    "elasticsearch",
    "redis",
    "nginx",
    "keepalived",
    "mysql",
    "java-service",
];

pub fn check() -> CheckResult {
    let mut sections = Vec::new();
    let mut summary_rows = Vec::new();
    let mut abnormal_rows = Vec::new();
    let mut errors = 0usize;
    let mut warnings = 0usize;

    for id in BUSINESS_CHECKS {
        let Some(result) = super::run_check_without_enrich(id) else {
            continue;
        };
        let findings = crate::anomaly::evaluate_check_result(&result);
        let error_count = findings.iter().filter(|f| f.level == "error").count();
        let warn_count = findings.iter().filter(|f| f.level == "warn").count();
        errors += error_count;
        warnings += warn_count;
        summary_rows.push(vec![
            result.id.clone(),
            result.name.clone(),
            format!("{:?}", result.status),
            error_count.to_string(),
            warn_count.to_string(),
            result.sections.len().to_string(),
        ]);
        for finding in findings {
            abnormal_rows.push(vec![
                result.name.clone(),
                finding.level,
                finding.category,
                finding.target,
                finding.summary,
                finding.suggestion,
            ]);
        }
    }

    sections.push(Section {
        title: "业务综合汇总".to_string(),
        icon: Some("BIZ".to_string()),
        description: Some(
            "统一汇总主机、中间件、Java 服务和安全网络检查，重点展示异常与处理建议".to_string(),
        ),
        items: vec![
            Item::Label {
                key: "检查项数".to_string(),
                value: summary_rows.len().to_string(),
                status: None,
            },
            Item::Label {
                key: "错误数".to_string(),
                value: errors.to_string(),
                status: Some(if errors > 0 { "error" } else { "ok" }.to_string()),
            },
            Item::Label {
                key: "警告数".to_string(),
                value: warnings.to_string(),
                status: Some(if warnings > 0 { "warn" } else { "ok" }.to_string()),
            },
        ],
    });

    sections.push(Section {
        title: "检查项状态".to_string(),
        icon: Some("SUMMARY".to_string()),
        description: None,
        items: vec![Item::Table {
            headers: vec![
                "ID".to_string(),
                "名称".to_string(),
                "状态".to_string(),
                "错误".to_string(),
                "警告".to_string(),
                "数据块".to_string(),
            ],
            rows: summary_rows,
            status: None,
        }],
    });

    sections.push(Section {
        title: "异常汇总".to_string(),
        icon: Some("ALERT".to_string()),
        description: Some(
            "所有异常均由规则引擎从结构化检查结果分析生成，可排序和展开查看".to_string(),
        ),
        items: if abnormal_rows.is_empty() {
            vec![Item::Success {
                text: "未发现业务综合异常".to_string(),
            }]
        } else {
            vec![Item::Table {
                headers: vec![
                    "来源".to_string(),
                    "级别".to_string(),
                    "分类".to_string(),
                    "对象".to_string(),
                    "概要".to_string(),
                    "处理建议".to_string(),
                ],
                rows: abnormal_rows,
                status: Some(if errors > 0 { "error" } else { "warn" }.to_string()),
            }]
        },
    });

    CheckResult {
        id: "business-check".to_string(),
        name: "业务综合检查".to_string(),
        description: "汇总系统、中间件、Java 服务、网络与安全的异常信息".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status: if errors > 0 {
            CheckStatus::Error
        } else if warnings > 0 {
            CheckStatus::Warn
        } else {
            CheckStatus::Ok
        },
        sections,
    }
}
