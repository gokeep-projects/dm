<script>
  import { onMount, onDestroy } from 'svelte';
  let activeSection = $state('introduction');
  let search = $state('');
  let observer = null;

  const sections = [
    { id: 'introduction', title: '产品介绍', icon: '◆', tag: 'intro' },
    { id: 'changelog', title: '新增功能', icon: '★', tag: 'new' },
    { id: 'installation', title: '安装部署', icon: '⬇', tag: 'install' },
    { id: 'cli-commands', title: 'CLI 命令', icon: '▶', tag: 'cli' },
    { id: 'web-interface', title: 'Web 界面', icon: '◉', tag: 'web' },
    { id: 'script-development', title: '脚本开发', icon: '⟨/⟩', tag: 'dev' },
    { id: 'configuration', title: '配置说明', icon: '⚙', tag: 'config' },
    { id: 'troubleshooting', title: '故障排除', icon: '!', tag: 'fix' },
    { id: 'api-reference', title: 'API 文档', icon: '⟨⟩', tag: 'api' },
    { id: 'shortcuts', title: '快捷键', icon: '⌘', tag: 'keys' },
    { id: 'faq', title: '常见问题', icon: '?', tag: 'faq' },
  ];

  const cards = [
    { section: 'introduction', title: '关于 DM', text: 'DM 是一款离线现场维护工具，默认暗黑主题，面向生产巡检、故障排查、日志分析、脚本执行和告警闭环。支持 CLI 和 Web 双模式，前端资源嵌入二进制。' },
    { section: 'introduction', title: '核心特性', type: 'list', items: [
      '单文件部署：编译后为单一二进制，前端资源嵌入其中',
      '零依赖：musl 静态编译，无需安装运行时环境',
      '双模式：CLI 命令行 + Web 图形界面',
      '结构化结果：脚本、常规检查、系统体检和导出数据均按 sections/items 渲染',
      '规则告警：检查结果统一进入异常规则引擎，告警持久化保存并低成本实时刷新',
      '一键导出：支持导出全部核心常规检查的完整结构化 JSON',
      '多平台：支持 x86_64 和 ARM 架构 Linux 系统',
      '可扩展：支持自定义脚本接入',
    ] },
    { section: 'introduction', title: '适用场景', text: '生产巡检·故障排查·日志分析·安全合规·服务治理' },
    { section: 'changelog', title: '仪表盘增强', type: 'list', items: [
      'CPU/内存/磁盘历史趋势图（SVG sparkline，30 个采样点）',
      '系统健康评分（基于 CPU/内存/磁盘/负载综合指标）',
      '网络接口信息（接收/发送字节数、错误数）',
      '磁盘挂载详情（每个挂载点的使用率、文件系统类型）',
      'Top 进程列表（按 CPU 占用排序）',
      'Swap 使用率显示',
      '负载均值可视化（1/5/15 分钟）',
      '快捷执行入口（选择脚本后进入结果页执行）',
      '执行历史持久化统计（成功/失败/最近执行）',
    ] },
    { section: 'changelog', title: '脚本详情增强', type: 'list', items: [
      '执行计时（实时显示已用时长）',
      '下载结果（结构化结果保存为 .json，原始日志保存为 .log）',
      '查看脚本源码（带行号、复制功能）',
      '查看执行历史（支持按时间/结果/耗时排序）',
      '复制脚本（创建副本）',
      '执行统计（总次数、成功率、平均耗时）',
      '参数输入表单（支持 string/number/boolean 类型）',
      '结构化结果渲染（label/bar/table/info/warning/error/success）',
      '键盘快捷键（Ctrl+Enter 执行、Ctrl+. 停止、Esc 返回）',
    ] },
    { section: 'changelog', title: '脚本中心增强', type: 'list', items: [
      '收藏功能（localStorage 持久化，star 按钮）',
      '排序选项（按名称/分类/执行次数/最近执行）',
      '快速执行按钮（hover 显示，点击跳转并自动运行）',
      '执行次数徽章（显示最近执行次数）',
      'Tooltip（悬停显示完整描述）',
    ] },
    { section: 'changelog', title: 'CLI 新增命令', type: 'list', items: [
      'dm stats <id> - 查看脚本执行统计',
      'dm duplicate <source_id> <new_id> - 复制脚本',
      'dm clean - 清空执行历史',
      'dm logs <id> - 查看脚本执行历史',
      'dm check <id> - 执行常规检查并渲染状态、分区、警告和错误明细',
      'dm check-export - 导出全部核心常规检查，可用 -o 写入文件',
      'dm java list/analyze/export - Java 进程发现、堆栈分析、PDF 报告和原始数据导出',
      'dm completions <shell> - 生成 bash/zsh/fish 等命令补全脚本',
      'dm list 新增"最近执行"列',
    ] },
    { section: 'changelog', title: '后端增强', type: 'list', items: [
      '脚本执行超时控制（kill_on_drop）',
      '执行历史持久化（SQLite：~/.dm/logs/dm.db）',
      '系统告警持久化（SQLite alert_events 表，支持活跃/历史查询）',
      '告警刷新采用后台周期扫描 + 请求触发陈旧缓存刷新，并通过互斥保护避免重复高 CPU 扫描',
      '系统体检支持后台任务进度：百分比、当前步骤、日志和结果同步到告警库',
      '新增 API：GET /api/checks/export（导出全部核心检查完整数据）',
      'WebSocket 心跳保活（每 30 秒 ping）',
      '新增 API：GET /api/scripts/:id/source（获取脚本源码）',
      '新增 API：POST /api/scripts/:id/duplicate（复制脚本）',
      '新增 API：GET /api/scripts/:id/stats（执行统计）',
      '新增 API：GET /api/dashboard/history（执行历史，支持筛选）',
      '新增 API：DELETE /api/dashboard/history（清空历史）',
      '新增 API：GET /api/alerts（告警列表，支持历史和限制数量）',
      '系统信息扩展（Swap、网络接口、磁盘挂载、Top 进程）',
    ] },
    { section: 'changelog', title: '通用改进', type: 'list', items: [
      'Ctrl+K 命令面板（快速搜索脚本）',
      '键盘快捷键（g+d/s/h 导航、[ 折叠侧边栏、? 帮助）',
      '使用指南搜索功能，顶部区域压缩，优先展示内容',
      'CSS 变量系统',
      '可访问性改进（focus-visible、aria-label）',
      '动画库（float、pulse-glow、shimmer、fadeIn、slideUp）',
    ] },
    { section: 'installation', title: '系统要求', type: 'list', items: [
      '操作系统：Linux (kernel 3.10+)',
      '架构：x86_64 或 aarch64',
      '依赖：bash、perl（如使用 perl 脚本）',
      '权限：安装时需要 root，运行时普通用户即可',
    ] },
    { section: 'installation', title: '安装步骤', type: 'code', code: '# 1. 解压安装包\nunzip dm-x86_64-unknown-linux-musl.zip\n\n# 2. 运行安装脚本（需要 root）\nsudo bash install.sh\n\n# 3. 验证安装\ndm list\n\n# 4. 启动服务\ndm serve\n\n# 5. 访问 Web 界面\n# 浏览器打开 http://localhost:3399' },
    { section: 'installation', title: '卸载', type: 'code', code: 'sudo bash uninstall.sh' },
    { section: 'cli-commands', title: 'dm list - 列出脚本', type: 'code', code: '# 列出所有脚本\ndm list\n\n# 按关键词搜索\ndm list -s security\n\n# 按分类筛选\ndm list -c 系统检查' },
    { section: 'cli-commands', title: 'dm info - 查看脚本详情', type: 'code', code: '# 查看脚本详细信息\ndm info security\n\n# 显示参数说明和使用示例\ndm info system' },
    { section: 'cli-commands', title: 'dm run - 执行脚本', type: 'code', code: '# 执行脚本\ndm run system\n\n# 带参数执行\ndm run log --tail 100\n\n# 持续监控模式\ndm run system -f' },
    { section: 'cli-commands', title: 'dm serve - 启动 Web 服务', type: 'code', code: '# 默认端口 3399，默认监听 0.0.0.0\ndm serve\n\n# 后台启动\ndm serve -d --bind 0.0.0.0 --port 3399\n\n# 指定端口\ndm serve --port 8080\n\n# 仅本机访问\ndm serve --bind 127.0.0.1 --port 3399' },
    { section: 'cli-commands', title: 'dm check / check-export - 常规检查', type: 'code', code: '# 渲染单个检查结果\ndm check system\n\ndm check elasticsearch\n\n# 输出原始 JSON\ndm check redis --json\n\n# 导出全部核心检查完整数据\ndm check-export -o dm-checks.json\n\n# 仅输出 JSON 到终端\ndm check-export --json' },
    { section: 'cli-commands', title: 'dm java - Java 堆栈实时分析', type: 'code', code: '# 列出运行中的 Java 进程\ndm java list\n\n# 快速分析指定 PID\ndm java analyze --pid 12345\n\n# 输出原始 JSON\ndm java analyze --pid 12345 --json\n\n# 导出原始运行时数据\ndm java export --pid 12345 --format raw --output java-raw.json\n\n# 导出终端报告\ndm java export --pid 12345 --format report --output java-report.md\n\n# 导出 PDF 报告\ndm java export --pid 12345 --format pdf --output java-report.pdf' },
    { section: 'cli-commands', title: 'dm completions - 命令补全', type: 'code', code: '# Bash\nmkdir -p ~/.local/share/bash-completion/completions\ndm completions bash > ~/.local/share/bash-completion/completions/dm\n\n# Zsh\ndm completions zsh > ~/.zfunc/_dm\n\n# Fish\ndm completions fish > ~/.config/fish/completions/dm.fish' },
    { section: 'web-interface', title: '仪表盘', text: '实时显示系统状态，包括 CPU/内存/磁盘使用率、负载均值、网络接口、磁盘挂载、Top 进程。快速执行会进入脚本结果页，确保展示执行后的最新结构化数据。' },
    { section: 'web-interface', title: '脚本中心', text: '浏览所有可用脚本，支持分类标签筛选、关键词搜索、收藏功能。每个脚本都有数字编号，可以直接输入编号或 #编号 快速定位。上传脚本默认只需要选择文件和标题，脚本 ID、分类、作者、说明等字段在高级配置中维护；用户脚本支持更新和删除。' },
    { section: 'web-interface', title: '脚本详情', text: '查看脚本的完整说明、参数列表、使用示例。执行完成后优先按 JSON sections/items 渲染页面结果；支持复制/下载结果 JSON、查看源码、查看可排序执行历史、复制脚本。' },
    { section: 'web-interface', title: '常规检查与系统体检', text: '常规检查卡片和列表均显示数字编号，可以输入编号、名称或 ID 快速搜索。点击任一常规检查会先自动执行最新检查，再渲染结构化结果；系统体检会创建后台任务，页面持续显示百分比、当前步骤、执行日志和阶段进度。完成后展示每个检查项的警告/错误数量，并把规则命中的异常同步到系统告警和告警铃铛。' },
    { section: 'web-interface', title: '服务管理', text: '服务管理列表支持横向拖动滚动，操作列固定在右侧。默认排序为有监听端口的服务优先，其次按 CPU/内存负载从高到低；点击 PID、服务名、进程、路径、端口、状态、CPU、内存、类型等列名可重新排序。搜索支持普通关键词和条件语法，例如 pid:1234、port:8080、listen:、name:nginx、process:java、path:/opt/app、category:中间件、status:running、cpu>10、mem>500。' },
    { section: 'web-interface', title: '流量分析', text: '流量分析仅支持 Linux raw socket。页面可选择网卡、协议和端口，更多 IP、域名、路径、请求头/正文过滤项默认收起；抓包记录按请求/响应或 flow 聚合，1 秒批量刷新，最新记录在前，最多渲染前 100 条。详情以模态框展示元数据、请求信息和响应信息，支持格式化和复制。支持导出 DM JSON 抓包文件，支持导入 DM JSON 和标准 PCAP 文件做基础 IPv4/TCP/UDP/HTTP 解码。HTTPS 正文是 TLS 加密内容，单纯网卡抓包只能看到连接元数据；如需解密，必须显式安装 DM CA 并让流量经过受控代理，Web 页面不会静默导入系统信任根。' },
    { section: 'web-interface', title: 'Java 堆栈实时分析', text: 'Java 分析页只显示运行中的 Java 进程。顶部过滤框支持按 PID、服务名、路径、监听端口搜索；实时跟踪会异步更新内存、线程、CPU、对象、热点、锁和异常综合结论。所有图表支持悬停查看数值，表格列头可排序，异常结论可点击查看详情。Web 与 CLI 均支持导出 PDF 报告和原始运行时数据。' },
    { section: 'web-interface', title: '系统告警', text: '后台异步分析资源、服务、日志、Journal、常规检查和脚本失败历史。告警持久化保存，列表默认显示活跃告警，可切换历史并按级别、对象、日志路径、时间和处理意见排序。' },
    { section: 'web-interface', title: '规则引擎导入', type: 'list', items: [
      '规则引擎页面支持查看、编辑、保存规则覆盖，保存后会立即刷新系统告警。',
      '批量导入前先点击“下载模板”，得到 <code>dm-rule-import-template.json</code>。',
      '导入文件必须是 <code>{"rules":[...]}</code> 或规则数组；<code>id</code> 必须来自内置规则。',
      '允许覆盖字段：<code>enabled</code>、<code>level</code>、<code>title</code>、<code>summary</code>、<code>suggestion</code>、<code>commands</code>。',
      '导入后后台会保存到数据库并实时应用到检查详情、系统告警、告警铃铛、系统体检和导出结果。',
    ] },
    { section: 'web-interface', title: '规则导入 JSON 示例', type: 'code', code: '{\n  "schema": "dm-rule-overrides/v1",\n  "rules": [\n    {\n      "id": "resource.cpu.warning",\n      "enabled": true,\n      "level": "error",\n      "title": "CPU 使用率需要立即处理",\n      "summary": "CPU 持续高于现场阈值，请立即定位 Top 进程",\n      "suggestion": "先确认是否有异常批处理、死循环或突发流量。",\n      "commands": ["top -o %CPU", "pidstat 1 5"]\n    }\n  ]\n}' },
    { section: 'web-interface', title: '导出检查信息', text: '常规检查页提供“导出全部检查”按钮，会下载核心检查的完整结构化 JSON，包括 sections/items、警告错误计数、命中告警和导出时间。' },
    { section: 'web-interface', title: '使用指南', text: '完整的工具使用文档，包含所有功能的详细说明。左侧导航支持快速跳转，搜索框可快速定位内容，支持键盘导航。' },
    { section: 'script-development', title: '目录结构', type: 'code', code: '~/.dm/scripts/\n├── mycheck/\n│   ├── mycheck.sh      # 脚本文件\n│   └── .dm.toml         # 元数据配置' },
    { section: 'script-development', title: '元数据格式 (.dm.toml)', type: 'code', code: 'name = "mycheck"\ndescription = "自定义检查脚本"\nfeature = "功能简述"\ncategory = "系统检查"\nversion = "1.0.0"\nauthor = "运维团队"\nexample = "dm run mycheck"\n\n# 可选：参数定义\n[[params]]\nname = "interval"\ndescription = "刷新间隔"\nrequired = false\ndefault = "5"' },
    { section: 'script-development', title: '支持的脚本语言', type: 'list', items: [
      'Bash (.sh)',
      'Perl (.pl)',
      'Python (.py)',
      'Node.js (.js)',
      '任何可执行文件（需有 shebang）',
    ] },
    { section: 'script-development', title: '脚本输出规范', text: '推荐输出 JSON。若返回 {sections:[...]}，Web 会直接按 label/bar/table/info/warning/error/success 渲染；若返回普通 object/array，Web 会自动包装成可展示的结果页。普通文本仍会作为兜底输出渲染。' },
    { section: 'configuration', title: '配置文件位置', type: 'list', items: [
      '系统配置：<code>/etc/dm/dm.toml</code>',
      '用户配置：<code>~/.dm/.dm.toml</code>',
      '项目配置：<code>./.dm.toml</code>',
    ] },
    { section: 'configuration', title: '环境变量', type: 'list', items: [
      '<code>DM_HOME</code>：DM 主目录路径',
      '<code>DM_PORT</code>：Web 服务端口',
      '<code>DM_BIND</code>：Web 服务监听地址',
      '<code>DM_LOG</code>：日志目录',
    ] },
    { section: 'configuration', title: 'Web 服务配置', text: '在 .dm.toml 中可配置 [server] 段，调整端口、绑定地址、最大并发等参数。' },
    { section: 'troubleshooting', title: '端口被占用', text: '使用以下命令查找并结束占用进程：', type: 'code', code: '# 查找占用端口的进程\nlsof -i :3399\n\n# 或使用 fuser\nfuser -k 3399/tcp' },
    { section: 'troubleshooting', title: '脚本无法执行', type: 'list', items: [
      '检查脚本是否有执行权限：<code>chmod +x script.sh</code>',
      '检查 shebang 行是否正确',
      '检查依赖程序是否已安装（如 perl、python3）',
      '查看日志：<code>~/.dm/logs/</code>',
    ] },
    { section: 'troubleshooting', title: 'Web 界面无法访问', type: 'list', items: [
      '确认服务已启动：<code>ps aux | grep "dm serve"</code>',
      '检查防火墙是否开放端口',
      '检查监听地址：默认 <code>0.0.0.0:3399</code>',
      '使用 <code>--bind 0.0.0.0</code> 监听所有接口',
    ] },
    { section: 'troubleshooting', title: '输出乱码', type: 'list', items: [
      '确认脚本输出 UTF-8 编码',
      '检查终端 locale 设置：<code>locale</code>',
      '设置 <code>export LANG=en_US.UTF-8</code>',
    ] },
    { section: 'api-reference', title: 'REST API', type: 'list', items: [
      '<code>GET /api/scripts</code> - 获取脚本列表（支持 search/category 参数）',
      '<code>GET /api/scripts/:id</code> - 获取脚本详情',
      '<code>GET /api/scripts/:id/source</code> - 获取脚本源码（content, line_count, size_bytes）',
      '<code>POST /api/scripts/:id/run</code> - 执行脚本（JSON body: {params, args}）',
      '<code>POST /api/scripts/:id/duplicate</code> - 复制脚本（JSON body: {new_id}）',
      '<code>POST /api/scripts/upload</code> - 上传用户维护脚本（multipart form-data）',
      '<code>GET /api/checks</code> - 获取所有常规检查项',
      '<code>GET /api/checks/:id</code> - 执行单个常规检查并返回完整结构化结果',
      '<code>GET /api/checks/export</code> - 导出全部核心常规检查完整数据',
      '<code>POST /api/health/full/start</code> - 启动后台系统体检任务',
      '<code>GET /api/health/full/:id</code> - 查询体检进度、百分比、日志和最终结果',
      '<code>GET /api/dashboard/stats</code> - 获取仪表盘统计（脚本数、执行数、成功/失败、最近执行）',
      '<code>GET /api/dashboard/history</code> - 获取执行历史（支持 script_id/limit 参数）',
      '<code>DELETE /api/dashboard/history</code> - 清空执行历史',
      '<code>GET /api/alerts</code> - 获取告警列表（支持 history/limit 参数）',
      '<code>GET /api/rules</code> - 获取规则目录和本地覆盖后的规则',
      '<code>PUT /api/rules/:id</code> - 保存单条规则覆盖并实时刷新告警',
      '<code>POST /api/rules/import</code> - 导入 JSON 规则覆盖文件并实时生效',
      '<code>GET /api/system/info</code> - 获取系统信息（CPU/内存/磁盘/负载/网络/进程）',
    ] },
    { section: 'api-reference', title: 'WebSocket API', text: '连接到 <code>ws://host/ws/exec/:id</code> 执行脚本。内置检查脚本会返回结构化结果；外部脚本返回 JSON 时会被转换为可渲染结果。', type: 'code', code: '// 发送执行命令\nws.send(JSON.stringify({\n  action: "run",\n  params: {},\n  args: []\n}));\n\n// 接收结果\nws.onmessage = (e) => {\n  const msg = JSON.parse(e.data);\n  if (msg.type === "result") {\n    const result = JSON.parse(msg.line);\n    // result.sections 可直接渲染为页面结果\n  }\n};' },
    { section: 'api-reference', title: '返回数据格式', text: '所有 API 返回 JSON 格式。错误时返回 {error: "msg"} + 4xx/5xx 状态码。' },
    { section: 'shortcuts', title: '全局快捷键', type: 'list', items: [
      '<code>g d</code> - 跳转到仪表盘',
      '<code>g s</code> - 跳转到脚本中心',
      '<code>g h</code> - 跳转到使用指南',
      '<code>?</code> - 显示快捷键帮助',
      '<code>Ctrl/Cmd + K</code> - 命令面板',
    ] },
    { section: 'shortcuts', title: '脚本执行页面', type: 'list', items: [
      '<code>Ctrl + Enter</code> - 执行脚本',
      '<code>Ctrl + .</code> - 停止执行',
      '<code>Esc</code> - 返回列表',
    ] },
    { section: 'shortcuts', title: '结果面板', type: 'list', items: [
      '<code>F11</code> - 全屏切换',
      '<code>复制</code> - 复制结构化结果 JSON 或原始输出',
      '<code>下载</code> - 结构化结果保存为 .json，原始输出保存为 .log',
      '<code>滚轮</code> - 上下滚动结果内容',
    ] },
    { section: 'faq', title: 'Q: 如何修改默认端口？', text: '使用 <code>--port</code> 参数：', type: 'code', code: 'dm serve --port 8080' },
    { section: 'faq', title: 'Q: 脚本数据存在哪里？', text: '脚本文件位于 <code>~/.dm/scripts/</code>，日志位于 <code>~/.dm/logs/</code>。' },
    { section: 'faq', title: 'Q: 如何备份所有脚本？', type: 'code', code: 'tar -czf dm-backup.tar.gz ~/.dm/scripts/' },
    { section: 'faq', title: 'Q: 可以在生产环境使用吗？', text: '可以。DM 设计用于生产运维，单二进制部署，零依赖，稳定可靠。建议在测试环境验证后再部署到生产。' },
    { section: 'faq', title: 'Q: 支持 Windows 吗？', text: '当前版本主要面向 Linux 系统（x86_64/aarch64），Windows 版本暂未支持。' },
  ];

  function cardMatches(card, q) {
    return card.title.toLowerCase().includes(q) ||
      (card.text && card.text.toLowerCase().includes(q)) ||
      (card.code && card.code.toLowerCase().includes(q)) ||
      (card.items && card.items.some(i => i.toLowerCase().includes(q)));
  }

  function sectionMatches(section, q) {
    return section.title.toLowerCase().includes(q) || section.tag.toLowerCase().includes(q);
  }

  let matchedCards = $derived.by(() => {
    if (!search.trim()) return null;
    const q = search.toLowerCase();
    return cards.filter(c => cardMatches(c, q));
  });

  let filtered = $derived.by(() => {
    if (!search.trim()) return sections;
    const q = search.toLowerCase();
    return sections.filter(s =>
      sectionMatches(s, q) ||
      cards.some(c => c.section === s.id && cardMatches(c, q))
    );
  });

  function scrollTo(id) {
    activeSection = id;
    document.getElementById(id)?.scrollIntoView({ behavior: 'smooth' });
  }

  function cardsBySection(sectionId) {
    if (matchedCards) {
      const q = search.toLowerCase();
      const section = sections.find(s => s.id === sectionId);
      if (section && sectionMatches(section, q)) return cards.filter(c => c.section === sectionId);
      return matchedCards.filter(c => c.section === sectionId);
    }
    return cards.filter(c => c.section === sectionId);
  }

  function searchResultCount() {
    return matchedCards ? matchedCards.length : cards.length;
  }

  onMount(() => {
    observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          activeSection = entry.target.id;
        }
      }
    }, { rootMargin: '-80px 0px -60% 0px', threshold: 0.1 });
    for (const s of sections) {
      const el = document.getElementById(s.id);
      if (el) observer.observe(el);
    }
  });

  onDestroy(() => {
    if (observer) { observer.disconnect(); observer = null; }
  });
</script>

<div class="help-page">
  <div class="help-layout">
    <nav class="help-nav">
      <div class="sidebar-search">
        <div class="search-box">
          <svg class="search-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="11" cy="11" r="8"/>
            <path d="m21 21-4.35-4.35" stroke-linecap="round"/>
          </svg>
          <input
            type="text"
            placeholder="搜索文档..."
            bind:value={search}
            class="search-input"
            aria-label="搜索文档" />
          {#if search}
            <button class="search-clear" onclick={() => search = ''} aria-label="清除搜索">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M18 6 6 18M6 6l12 12" stroke-linecap="round" />
              </svg>
            </button>
          {/if}
        </div>
        {#if search}
          <div class="search-result-hint">命中 {searchResultCount()} 条内容</div>
        {/if}
      </div>
      {#each filtered as s (s.id)}
        <button class="nav-link" class:active={activeSection === s.id} onclick={() => scrollTo(s.id)}>
          <span class="nav-icon">{s.icon}</span>
          <span>{s.title}</span>
        </button>
      {/each}
    </nav>

    <div class="help-content">
      {#each filtered as s (s.id)}
        <section id={s.id} class="help-section">
          <h2>
            <span class="section-icon">{s.icon}</span>
            {s.title}
          </h2>
          {#each cardsBySection(s.id) as card, i (s.id + i)}
            <div class="card">
              <h3>{card.title}</h3>
              {#if card.type === 'list'}
                <ul class="feature-list">
                  {#each card.items as item}
                    <li>{@html item}</li>
                  {/each}
                </ul>
              {:else if card.type === 'code'}
                <div class="code-block">
                  <pre><code>{card.code}</code></pre>
                </div>
                {#if card.text}
                  <p class="help-text" style="margin-top: 10px">{@html card.text}</p>
                {/if}
              {:else}
                <p class="help-text">{@html card.text || ''}</p>
              {/if}
            </div>
          {/each}
        </section>
      {/each}

      {#if filtered.length === 0}
        <div class="empty-state">
          <div class="empty-icon">🔍</div>
          <div class="empty-text">没有匹配 "{search}" 的内容</div>
          <button class="empty-btn" onclick={() => search = ''}>清除搜索</button>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .help-page { max-width: 1200px; margin: 0 auto; }
  .search-box { position: relative; width: 100%; flex-shrink: 0; }
  .search-icon { position: absolute; left: 14px; top: 50%; transform: translateY(-50%); color: #4b5563; }
  .search-input { width: 100%; padding: 9px 34px 9px 38px; background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 10px; font-size: 13px; color: #e2e8f0; transition: all 0.2s; outline: none; box-sizing: border-box; }
  .search-input::placeholder { color: #4b5563; }
  .search-input:focus { border-color: rgba(34, 211, 238, 0.3); box-shadow: 0 0 0 3px rgba(34, 211, 238, 0.08); }
  .search-clear { position: absolute; right: 8px; top: 50%; transform: translateY(-50%); background: none; border: none; color: #4b5563; padding: 4px; border-radius: 4px; cursor: pointer; display: flex; align-items: center; justify-content: center; }
  .search-clear:hover { color: #94a3b8; background: rgba(255, 255, 255, 0.05); }
  .help-layout { display: flex; gap: 20px; height: calc(100vh - 72px); }
  .help-nav { width: 220px; flex-shrink: 0; position: sticky; top: 0; align-self: flex-start; display: flex; flex-direction: column; gap: 4px; max-height: calc(100vh - 72px); overflow-y: auto; padding-right: 8px; }
  .sidebar-search { position: sticky; top: 0; z-index: 3; padding: 0 0 10px; margin-bottom: 4px; background: var(--bg-primary); }
  .search-result-hint { margin-top: 7px; padding: 0 2px; color: #64748b; font-size: 11px; font-family: var(--theme-font-family-mono); }
  .nav-link { display: flex; align-items: center; gap: 10px; padding: 10px 14px; border-radius: 10px; border: none; background: transparent; color: #6b7280; font-size: 13px; font-weight: 500; cursor: pointer; transition: all 0.2s; text-align: left; }
  .nav-link:hover { background: rgba(255, 255, 255, 0.04); color: #94a3b8; }
  .nav-link.active { background: rgba(34, 211, 238, 0.08); color: #22d3ee; }
  .nav-icon { font-size: 15px; width: 20px; text-align: center; flex-shrink: 0; }
  .help-content { flex: 1; min-width: 0; overflow-y: auto; max-height: calc(100vh - 72px); padding-right: 8px; }
  .help-section { margin-bottom: 32px; scroll-margin-top: 20px; }
  .help-section h2 { font-size: 16px; font-weight: 600; color: #f1f5f9; margin-bottom: 16px; padding-bottom: 8px; border-bottom: 1px solid rgba(255, 255, 255, 0.06); display: flex; align-items: center; gap: 10px; }
  .section-icon { color: #22d3ee; font-size: 18px; }
  .card { background: rgba(15, 17, 23, 0.7); border: 1px solid rgba(255, 255, 255, 0.06); border-radius: 12px; padding: 16px; margin-bottom: 12px; transition: border-color 0.2s; }
  .card:hover { border-color: rgba(34, 211, 238, 0.15); }
  .card h3 { font-size: 14px; font-weight: 600; color: #e2e8f0; margin-bottom: 10px; }
  .code-block { background: rgba(0, 0, 0, 0.3); border: 1px solid rgba(255, 255, 255, 0.04); border-radius: 8px; padding: 12px; overflow-x: auto; position: relative; }
  .code-block pre { margin: 0; }
  .code-block code { font-family: var(--theme-font-family-mono); font-size: 12px; color: #94a3b8; line-height: 1.6; white-space: pre; }
  .help-text { font-size: 13px; color: #94a3b8; line-height: 1.6; margin: 0; }
  .help-text :global(code) { font-family: var(--theme-font-family-mono); font-size: 12px; color: #22d3ee; background: rgba(34, 211, 238, 0.1); padding: 2px 6px; border-radius: 4px; }
  .feature-list { margin: 0; padding-left: 20px; list-style: none; }
  .feature-list li { font-size: 13px; color: #94a3b8; line-height: 1.8; position: relative; }
  .feature-list li::before { content: '•'; color: #22d3ee; position: absolute; left: -16px; }
  .feature-list :global(code) { font-family: var(--theme-font-family-mono); font-size: 12px; color: #22d3ee; background: rgba(34, 211, 238, 0.1); padding: 2px 6px; border-radius: 4px; }
  .empty-state { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 12px; padding: 80px 0; color: #6b7280; }
  .empty-icon { font-size: 48px; opacity: 0.5; }
  .empty-text { font-size: 14px; }
  .empty-btn { margin-top: 8px; padding: 6px 14px; background: rgba(34, 211, 238, 0.1); border: 1px solid rgba(34, 211, 238, 0.2); color: #22d3ee; border-radius: 6px; font-size: 12px; cursor: pointer; transition: all 0.2s; }
  .empty-btn:hover { background: rgba(34, 211, 238, 0.15); }
  @media (max-width: 768px) {
    .help-layout { flex-direction: column; }
    .help-nav { width: 100%; flex-direction: row; overflow-x: auto; position: static; max-height: none; align-items: center; }
    .sidebar-search { position: static; width: min(260px, 80vw); flex: 0 0 auto; padding: 0; margin: 0 6px 0 0; background: transparent; }
    .nav-link { white-space: nowrap; }
  }
</style>
