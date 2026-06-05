use super::*;

pub fn check() -> CheckResult {
    let mut sections = vec![];

    // 汇总检查
    let mut issues = 0;
    let mut warnings = 0;

    // 系统检查
    let sys_result = crate::checks::system::check();
    let mut engine_findings = crate::anomaly::evaluate_check_result(&sys_result);
    for section in &sys_result.sections {
        for item in &section.items {
            count_item_status(item, &mut issues, &mut warnings);
        }
    }

    // 安全检查
    let sec_result = crate::checks::security::check();
    engine_findings.extend(crate::anomaly::evaluate_check_result(&sec_result));
    for section in &sec_result.sections {
        for item in &section.items {
            count_item_status(item, &mut issues, &mut warnings);
        }
    }

    for finding in &engine_findings {
        if finding.level == "error" {
            issues += 1;
        } else if finding.level == "warn" {
            warnings += 1;
        }
    }

    let status = if issues > 0 {
        CheckStatus::Error
    } else if warnings > 0 {
        CheckStatus::Warn
    } else {
        CheckStatus::Ok
    };

    // 摘要
    sections.push(Section {
        title: "检查摘要".to_string(),
        icon: Some("📊".to_string()),
        description: None,
        items: vec![
            Item::Label {
                key: "问题数".to_string(),
                value: issues.to_string(),
                status: Some(if issues > 0 { "error" } else { "ok" }.to_string()),
            },
            Item::Label {
                key: "警告数".to_string(),
                value: warnings.to_string(),
                status: Some(if warnings > 0 { "warn" } else { "ok" }.to_string()),
            },
            Item::Label {
                key: "检查项".to_string(),
                value: "系统/安全/网络/资源/服务".to_string(),
                status: None,
            },
        ],
    });

    // 系统摘要
    let sys_info = sys_result
        .sections
        .iter()
        .flat_map(|s| &s.items)
        .filter_map(|i| match i {
            Item::Label { key, value, .. } => Some((key.clone(), value.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();

    let sys_items: Vec<Item> = sys_info
        .into_iter()
        .map(|(k, v)| Item::Label {
            key: k,
            value: v,
            status: None,
        })
        .collect();

    sections.push(Section {
        title: "系统摘要".to_string(),
        icon: Some("🖥️".to_string()),
        description: None,
        items: sys_items,
    });

    if let Some(section) = crate::anomaly::findings_section(&engine_findings) {
        sections.push(section);
    }

    // 建议
    let mut recommendations = Vec::new();
    if issues > 0 {
        recommendations.push(Item::Warning {
            text: format!("发现 {} 个问题需要处理", issues),
        });
    }
    if warnings > 0 {
        recommendations.push(Item::Info {
            text: format!("发现 {} 个警告", warnings),
        });
    }
    if issues == 0 && warnings == 0 {
        recommendations.push(Item::Success {
            text: "系统运行正常".to_string(),
        });
    }

    sections.push(Section {
        title: "建议".to_string(),
        icon: Some("💡".to_string()),
        description: None,
        items: recommendations,
    });

    CheckResult {
        id: "smart-check".to_string(),
        name: "智能全量体检".to_string(),
        description: "智能综合检查：系统配置/服务/安全".to_string(),
        category: "常规检查".to_string(),
        version: "1.0.0".to_string(),
        timestamp: String::new(),
        duration_ms: 0,
        status,
        sections,
    }
}

fn count_item_status(item: &Item, issues: &mut i32, warnings: &mut i32) {
    match item {
        Item::Bar {
            status: Some(s), ..
        }
        | Item::Label {
            status: Some(s), ..
        } => {
            if s == "error" {
                *issues += 1;
            } else if s == "warn" {
                *warnings += 1;
            }
        }
        Item::Warning { .. } => *warnings += 1,
        Item::Error { .. } => *issues += 1,
        Item::Finding { level, .. } => {
            if level == "error" {
                *issues += 1;
            } else if level == "warn" {
                *warnings += 1;
            }
        }
        _ => {}
    }
}
