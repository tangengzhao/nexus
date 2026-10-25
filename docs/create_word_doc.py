#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
生成 Nexus HSB 软件功能列表 Word 文档
前 7 章为系统功能界面（含截图），第 8 章起为后台核心功能说明。
"""

from pathlib import Path
from docx import Document
from docx.shared import Pt, Inches, RGBColor, Cm
from docx.enum.text import WD_ALIGN_PARAGRAPH
from docx.enum.table import WD_TABLE_ALIGNMENT
from docx.oxml.ns import qn

ROOT_DIR = Path(__file__).resolve().parent.parent
SCREENSHOT_DIR = Path("/tmp/hsb_screenshots")
OUTPUT_PATH = ROOT_DIR / "docs" / "nexusHSB软件功能列表.docx"


# ──────────────────────── 样式与工具函数 ────────────────────────


def setup_styles(doc: Document) -> None:
    style = doc.styles["Normal"]
    style.font.name = "微软雅黑"
    style.font.size = Pt(11)
    style.element.rPr.rFonts.set(qn("w:eastAsia"), "微软雅黑")
    style.paragraph_format.space_after = Pt(6)
    style.paragraph_format.line_spacing = 1.25

    for level in range(1, 4):
        heading = doc.styles[f"Heading {level}"]
        heading.font.name = "微软雅黑"
        heading.element.rPr.rFonts.set(qn("w:eastAsia"), "微软雅黑")
        heading.font.color.rgb = RGBColor(0x1A, 0x3C, 0x6E)


def add_table(doc: Document, headers: list[str], rows: list[list[str]]) -> None:
    table = doc.add_table(rows=1 + len(rows), cols=len(headers))
    table.style = "Light Grid Accent 1"
    table.alignment = WD_TABLE_ALIGNMENT.CENTER
    for i, h in enumerate(headers):
        cell = table.rows[0].cells[i]
        cell.text = h
        for p in cell.paragraphs:
            for r in p.runs:
                r.bold = True
                r.font.size = Pt(10)
    for r_idx, row in enumerate(rows):
        for c_idx, val in enumerate(row):
            cell = table.rows[r_idx + 1].cells[c_idx]
            cell.text = val
            for p in cell.paragraphs:
                for r in p.runs:
                    r.font.size = Pt(10)
    doc.add_paragraph()


def add_bullet(doc: Document, text: str, level: int = 0) -> None:
    p = doc.add_paragraph(text, style="List Bullet")
    p.paragraph_format.left_indent = Cm(1.27 * (level + 1))
    for r in p.runs:
        r.font.size = Pt(11)


def add_screenshot(doc: Document, filename: str, caption: str) -> None:
    filepath = SCREENSHOT_DIR / filename
    if filepath.exists():
        paragraph = doc.add_paragraph()
        paragraph.alignment = WD_ALIGN_PARAGRAPH.CENTER
        run = paragraph.add_run()
        run.add_picture(str(filepath), width=Inches(6))
        cap = doc.add_paragraph()
        cap.alignment = WD_ALIGN_PARAGRAPH.CENTER
        cr = cap.add_run(caption)
        cr.font.size = Pt(9)
        cr.font.color.rgb = RGBColor(128, 128, 128)
    else:
        p = doc.add_paragraph(f"[截图未找到: {filename}]")
        p.alignment = WD_ALIGN_PARAGRAPH.CENTER
        for r in p.runs:
            r.font.color.rgb = RGBColor(180, 180, 180)


# ──────────────────────── 第一部分：系统功能界面 ────────────────────────


def build_part1_ui(doc: Document) -> None:
    """第 1-7 章：系统功能界面（含截图）"""

    # ── 1. 运行总览 ──
    doc.add_heading("1  运行总览", level=1)
    doc.add_paragraph(
        "运行总览页面显示系统当前的运行状态、组件状态、消息统计等核心指标，"
        "帮助运维人员快速了解平台健康状况。"
    )
    add_screenshot(doc, "02_系统状态.png", "图 1-1：系统运行总览页面")

    doc.add_heading("系统状态数据", level=3)
    add_bullet(doc, "版本号：显示当前 HSB Server 版本")
    add_bullet(doc, "运行时长：系统启动至今的秒数")
    add_bullet(doc, "活跃路由数 / 活跃端点数")
    add_bullet(doc, "死信队列大小")
    add_bullet(doc, "消息统计：已接收 / 已发送 / 失败数")

    # ── 2. 端点管理 ──
    doc.add_heading("2  端点管理", level=1)
    doc.add_paragraph(
        "端点管理用于配置和维护外部系统的连接信息，包括主机地址、端口、协议、"
        "认证方式等。每个端点代表一个具体的业务系统连接。"
    )

    doc.add_heading("主要功能", level=2)
    add_bullet(doc, "创建端点：配置连接参数、认证信息、安全策略")
    add_bullet(doc, "查询端点：按系统类型、协议等条件查询")
    add_bullet(doc, "更新端点：修改连接配置、启用/禁用端点")
    add_bullet(doc, "端点健康检查：实时监控端点连通性")
    add_bullet(doc, "版本管理：记录端点配置变更历史")
    add_bullet(doc, "安全配置：TLS 配置、IP 白名单、凭证轮换")

    add_screenshot(doc, "03_端点管理.png", "图 2-1：端点管理页面")

    # ── 3. 路由规则 ──
    doc.add_heading("3  路由规则", level=1)
    doc.add_paragraph(
        "路由规则定义了消息从源系统到目标端点的流转规则。支持多种匹配条件和投递模式。"
    )

    doc.add_heading("主要功能", level=2)
    add_bullet(doc, "创建路由：配置源系统、消息类型、匹配条件、目标端点")
    add_bullet(doc, "条件匹配：支持源系统、协议、消息类型、Header、Payload 等多维度匹配")
    add_bullet(doc, "投递模式：支持 At Least Once、At Most Once、Exactly Once")
    add_bullet(doc, "多目标路由：支持主备目标、权重分配")
    add_bullet(doc, "路由优先级：支持多路由并发匹配")
    add_bullet(doc, "启用/禁用：灵活控制路由生效状态")

    add_screenshot(doc, "04_路由规则.png", "图 3-1：路由规则页面")

    # ── 4. 消息管理 ──
    doc.add_heading("4  消息管理", level=1)
    doc.add_paragraph(
        "消息管理提供历史消息查询、消息详情查看、消息重放等功能，"
        "支持按源系统、目标系统、消息类型、状态等条件筛选。"
    )

    doc.add_heading("主要功能", level=2)
    add_bullet(doc, "消息列表：查看历史消息记录")
    add_bullet(doc, "消息详情：查看完整消息内容、路由记录、投递结果")
    add_bullet(doc, "消息重放：重新发送失败或需要补偿的消息")
    add_bullet(doc, "消息追踪：查看消息完整处理链路")
    add_bullet(doc, "状态筛选：按消息状态（Received、Processing、Completed、Failed）筛选")

    add_screenshot(doc, "05_消息管理.png", "图 4-1：消息管理页面")

    # ── 5. 死信队列 ──
    doc.add_heading("5  死信队列（DLQ）", level=1)
    doc.add_paragraph(
        "死信队列用于存放投递失败的消息，支持查询、重放和清理操作。"
        "失败原因包括最大重试次数、目标不可用、解析错误等。"
    )

    doc.add_heading("主要功能", level=2)
    add_bullet(doc, "DLQ 查询：查看所有死信消息")
    add_bullet(doc, "死信详情：查看失败原因、原始消息、重试历史")
    add_bullet(doc, "消息重放：重新处理死信消息")
    add_bullet(doc, "消息删除：清理不需要的死信消息")
    add_bullet(doc, "统计分析：按原因、时间范围统计死信数据")

    add_screenshot(doc, "06_死信队列.png", "图 5-1：死信队列页面")

    # ── 6. 审计追踪 ──
    doc.add_heading("6  审计追踪", level=1)
    doc.add_paragraph(
        "审计追踪记录所有消息处理的审计事件，支持按 trace_id、message_id、事件类型查询。"
        "用于问题排查、合规审计和流程追踪。"
    )

    doc.add_heading("主要功能", level=2)
    add_bullet(doc, "审计事件查询：查看所有审计记录")
    add_bullet(doc, "消息链路追踪：按 message_id 查看完整处理链路")
    add_bullet(doc, "失败事件筛选：快速定位问题消息")
    add_bullet(doc, "多维度查询：按源系统、时间范围、事件类型筛选")
    add_bullet(doc, "审计日志：记录消息在各组件的处理过程")

    add_screenshot(doc, "07_审计追踪.png", "图 6-1：审计追踪页面")

    # ── 7. 管理台首页 ──
    doc.add_heading("7  管理台首页", level=1)
    doc.add_paragraph(
        "管理台首页提供快速导航和系统概览，支持跳转到各个功能模块。"
    )
    add_screenshot(doc, "01_首页.png", "图 7-1：管理台首页")


# ──────────────────────── 第二部分：后台核心功能 ────────────────────────


def build_part2_backend(doc: Document) -> None:
    """第 8-17 章：后台核心功能说明"""

    # ── 8. 多协议接入与适配 ──
    doc.add_heading("8  多协议接入与适配", level=1)

    doc.add_heading("8.1  支持的医疗协议", level=2)
    add_table(doc,
        ["协议", "说明"],
        [
            ["HL7 v2.x", "支持 HL7 2.1 ~ 2.10 全版本；MLLP 帧封装/解封；管道分隔解析；"
                         "MSH/PID/PV1/ORC/OBR/OBX 等标准段解析；自动生成 ACK/NAK 应答"],
            ["HL7 v3 (CDA)", "XML 格式 HL7 v3 消息解析与序列化；urn:hl7-org:v3 命名空间验证；"
                             "interactionId 提取"],
            ["FHIR R5", "FHIR JSON 资源解析；支持 DSTU2/STU3/R4/R4B/R5 版本；"
                        "Bundle 操作；15+ 种资源类型自动识别；OperationOutcome 错误处理"],
            ["DICOM", "二进制 DICOM 解析（基于 dicom-rs）；患者/检查/序列/实例信息提取；"
                      "DIMSE 服务定义（C-STORE/C-FIND/C-MOVE 等 10 种）；"
                      "传输语法支持（Implicit/Explicit VR、JPEG、JPEG 2000）"],
            ["SOAP", "SOAP 1.1/1.2 XML 解析与生成；WS-Addressing 消息头；"
                     "WS-Security 认证（UsernameToken）；SOAP Fault 生成"],
            ["自定义协议", "通过自定义协议目录定义字段结构；支持自定义 transport_hint、"
                          "content_type、sample_payload"],
        ])

    doc.add_heading("8.2  协议自动检测", level=2)
    doc.add_paragraph("HTTP 入站消息无需手动指定协议，系统按优先级自动识别：")
    add_bullet(doc, "显式指定：请求头 X-HSB-Protocol 直接声明")
    add_bullet(doc, "Content-Type 探测：application/fhir+* → FHIR，application/soap+* → SOAP，"
                    "application/dicom → DICOM，application/json → 通用 HTTP")
    add_bullet(doc, "消息体探测：含 urn:hl7-org:v3 → HL7v3，含 <envelope> → SOAP，"
                    "MSH| 开头 → HL7v2")
    add_bullet(doc, "默认回退：归为 HTTP 协议")

    doc.add_heading("8.3  协议转换", level=2)
    doc.add_paragraph(
        "所有入站消息经协议适配器解析为统一 Canonical Message（JSON payload），"
        "再由目标端适配器序列化为目标协议格式，实现任意协议间的转换："
    )
    add_bullet(doc, "HL7 v2 ↔ FHIR：管道分隔与 FHIR JSON 互转")
    add_bullet(doc, "DICOM → FHIR：DICOM 元素提取为 JSON，转换为 FHIR ImagingStudy 资源")
    add_bullet(doc, "SOAP ↔ FHIR / HL7 v2：SOAP XML Body 解析后转换为目标协议")
    add_bullet(doc, "任意 ↔ 任意：通过中间 Canonical Message 格式实现任意协议互转")

    # ── 9. 多传输通道与接入方式 ──
    doc.add_heading("9  多传输通道与接入方式", level=1)

    doc.add_heading("9.1  支持的传输通道", level=2)
    add_table(doc,
        ["传输通道", "入站", "出站", "说明"],
        [
            ["HTTP/HTTPS", "✅", "✅", "axum 服务器 / reqwest 客户端；连接池、TLS、代理、自定义请求头"],
            ["TCP/MLLP",   "✅", "✅", "标准 HL7 MLLP 端口 2575；支持裸 TCP 模式；MLLP 流式帧解析"],
            ["gRPC",       "✅", "✅", "tonic 双向流；TLS；连接池；重试策略；健康检查"],
            ["RabbitMQ",   "✅", "✅", "Direct/Fanout/Topic/Headers 交换机；Ack/Nack/Reject；死信路由"],
            ["Kafka",      "✅", "✅", "SASL 认证；Consumer Group；FutureProducer；Topic 自动解析"],
            ["NATS/JetStream", "✅", "✅", "Request-Reply；ExactlyOnce 去重窗口；流保留策略；Pull Consumer"],
            ["Webhook",    "—",  "✅", "经 HTTP 传输 POST 回调；支持自定义 Secret 签名验证"],
        ])

    doc.add_heading("9.2  多种接入方式", level=2)
    doc.add_paragraph("外部系统可通过以下方式接入 HSB：")
    add_bullet(doc, "REST API：POST /api/messages/inbound，支持 JSON / FHIR / SOAP 等多种 Content-Type")
    add_bullet(doc, "TCP/MLLP 长连接：标准 HL7v2 MLLP 协议接入，端口 2575")
    add_bullet(doc, "gRPC 调用：端口 10051，支持 SSO 集成认证")
    add_bullet(doc, "消息队列订阅：RabbitMQ / Kafka / NATS 消费者模式，自动拉取消息")
    add_bullet(doc, "Webhook 推送：HSB 主动向注册的 Webhook URL 推送消息")

    # ── 10. 一对多与多对多连接路由 ──
    doc.add_heading("10  一对多与多对多连接路由", level=1)

    doc.add_heading("10.1  路由模型", level=2)
    doc.add_paragraph("路由规则（Route）定义消息从源系统到目标端点的流转规则：")
    add_bullet(doc, "源匹配：按系统 ID（精确或通配符 *）、协议类型、消息类型正则匹配")
    add_bullet(doc, "条件匹配：12 种运算符（Equals / NotEquals / Contains / Regex / In / NotIn 等），"
                    "可匹配 Header、Payload（JSON dot-path）、RawPayload、Metadata 字段")
    add_bullet(doc, "优先级：多路由同时匹配时，按优先级降序选取")

    doc.add_heading("10.2  一对多连接（1 → N）", level=2)
    doc.add_paragraph("一条消息可同时路由到多个目标端点：")
    add_bullet(doc, "多目标路由：每条 Route 可配置多个 RouteTarget，消息分发给所有目标")
    add_bullet(doc, "主备目标：primary 标记主目标，fallback 标记备目标，主目标失败时自动切换")
    add_bullet(doc, "权重分配：每个目标可设置 weight，用于加权负载均衡")
    add_bullet(doc, "目标级转换：每个目标可指定独立的 transformer_ids，实现同一消息针对不同系统的格式转换")
    doc.add_paragraph("典型场景：HIS 发出的患者登记消息同时推送给 LIS、RIS、EMR 等多个下游系统。")

    doc.add_heading("10.3  多对多连接（N → M）", level=2)
    doc.add_paragraph("多个源系统的消息可通过不同路由规则分发到多个目标系统：")
    add_bullet(doc, "路由表：支持配置任意数量的路由规则，每条规则独立匹配源和目标")
    add_bullet(doc, "通配源：源系统匹配支持通配符 *，一条规则即可匹配所有源系统的特定消息类型")
    add_bullet(doc, "并发路由匹配：入站消息同时与所有已启用路由规则匹配，命中多条时按优先级排序全部执行")
    add_bullet(doc, "批量派发：dispatch_batch 将消息派发到所有匹配路由的所有目标")
    doc.add_paragraph("典型场景：HIS、LIS、RIS 等多个系统均向 HSB 发送消息，由不同路由规则分别转发给对应的目标系统。")

    doc.add_heading("10.4  投递保证模式", level=2)
    add_table(doc,
        ["模式", "说明"],
        [
            ["At Most Once",    "最多一次，发后即忘，不重试"],
            ["At Least Once（默认）", "至少一次，失败后依次尝试主备目标，确保投递成功"],
            ["Exactly Once",    "精确一次，基于幂等存储去重（route_id:endpoint_id:correlation_id 组合键），防止重复投递"],
        ])

    # ── 11. 消息处理引擎 ──
    doc.add_heading("11  消息处理引擎", level=1)

    doc.add_heading("11.1  处理管道", level=2)
    doc.add_paragraph("消息经过可配置的处理阶段，按优先级顺序执行：")
    add_table(doc,
        ["阶段", "优先级", "说明"],
        [
            ["日志记录",     "0", "记录消息 ID、来源、协议、类型"],
            ["指标采集",     "1", "运行时指标收集"],
            ["验证",         "5", "严格模式（拒绝无效消息）/ 宽松模式（仅警告）"],
            ["转换",        "10", "字段映射、Header 操作等"],
        ])
    add_bullet(doc, "支持快速失败模式（默认）或继续执行模式")
    add_bullet(doc, "每个阶段记录处理耗时，形成完整处理链路")

    doc.add_heading("11.2  消息转换器", level=2)
    add_table(doc,
        ["转换器", "功能"],
        [
            ["FieldMappingTransformer", "JSON 字段映射，支持源路径→目标路径 dot-path、必填/默认值/转换表达式"],
            ["HeaderTransformer",       "消息头操作：Set / Remove / Rename / Copy"],
            ["PassthroughTransformer",  "透传，不做任何转换"],
            ["JsonToXmlTransformer",    "JSON → XML 转换（预留）"],
            ["XmlToJsonTransformer",    "XML → JSON 转换（预留）"],
        ])
    doc.add_paragraph("转换器链（TransformerChain）按顺序串联执行，每步输出作为下步输入。")

    # ── 12. 可靠性保障 ──
    doc.add_heading("12  可靠性保障", level=1)

    doc.add_heading("12.1  熔断器（Circuit Breaker）", level=2)
    add_bullet(doc, "三状态模型：Closed（正常）→ Open（熔断，拒绝请求）→ HalfOpen（探针，放行少量请求）")
    add_bullet(doc, "状态转换：连续失败 ≥ 阈值（默认 5 次）→ Open；等待恢复时间（默认 60s）→ HalfOpen；连续成功 ≥ 阈值（默认 3 次）→ Closed")
    add_bullet(doc, "手动重置：通过管理 API 手动重置熔断器")
    add_bullet(doc, "独立实例：每个端点独立熔断器，互不影响")

    doc.add_heading("12.2  重试机制", level=2)
    add_table(doc,
        ["策略", "公式", "默认参数"],
        [
            ["指数退避（默认）", "base × multiplier^attempt", "base=1000ms, max=300s, multiplier=2.0"],
            ["固定间隔",         "恒定 delay_ms",             "—"],
            ["线性递增",         "initial + increment × attempt", "—"],
            ["自定义序列",       "按用户指定序列执行",          "—"],
        ])
    add_bullet(doc, "默认最大重试 3 次")
    add_bullet(doc, "启用抖动（jitter），避免重试风暴")
    add_bullet(doc, "仅对可重试错误执行重试")

    doc.add_heading("12.3  死信队列（DLQ）", level=2)
    add_bullet(doc, "9 种死信原因：MaxRetriesExceeded、MessageExpired、Unroutable、TargetUnavailable、ParseError、ValidationFailed、TransformFailed、ManuallyMoved、Other")
    add_bullet(doc, "DLQ 操作：查询（多维过滤）、重放（重新处理）、删除、统计（按原因/源系统/目标系统分组）")
    add_bullet(doc, "自动清理：按消息存活时间自动清理（默认保留 7 天）")
    add_bullet(doc, "容量保护：内存 DLQ 有上限，满时淘汰最老消息")

    doc.add_heading("12.4  消息优先级队列", level=2)
    add_bullet(doc, "五级优先级：Low(0)、Normal(1)、High(2)、Urgent(3)、Critical(4)")
    add_bullet(doc, "三级分桶：High (≥7) / Medium (≥4) / Low (其他)")
    add_bullet(doc, "出队顺序：High → Medium → Low")

    # ── 13. 工作流引擎 ──
    doc.add_heading("13  工作流引擎", level=1)

    doc.add_heading("13.1  步骤类型", level=2)
    add_table(doc,
        ["类型", "说明"],
        [
            ["Send",        "向指定端点发送消息，可附加转换器"],
            ["Receive",     "等待响应消息，可设置超时"],
            ["Transform",   "应用转换器链"],
            ["Parallel",    "并行执行多个分支，支持 All/Any/N 汇合模式"],
            ["Choice",      "条件分支，支持默认分支"],
            ["Script",      "内联脚本执行（JavaScript / Lua / Expression）"],
            ["SubWorkflow", "嵌套调用另一个工作流"],
            ["Wait",        "延时等待"],
            ["Log",         "记录诊断日志"],
        ])

    doc.add_heading("13.2  编排能力", level=2)
    add_bullet(doc, "条件路由：步骤可配置 next_steps，基于条件选择下一步")
    add_bullet(doc, "并行执行：Parallel 步骤可同时执行多个分支")
    add_bullet(doc, "Saga 补偿：每个步骤可定义补偿步骤，失败时按配置策略回滚")
    add_bullet(doc, "补偿策略：Sequential（逆序回滚）/ Parallel（并行回滚）/ Manual（手动触发）")
    add_bullet(doc, "生命周期控制：Pause / Resume / Cancel / Compensate")

    doc.add_heading("13.3  实例管理", level=2)
    add_bullet(doc, "工作流实例持久化（状态、变量、步骤历史）")
    add_bullet(doc, "支持查询、暂停、恢复、取消、补偿操作")
    add_bullet(doc, "实例超时控制（默认 3600s）")

    # ── 14. 持久化与存储 ──
    doc.add_heading("14  持久化与存储", level=1)

    doc.add_heading("14.1  双层存储架构", level=2)
    add_table(doc,
        ["层级", "技术", "用途"],
        [
            ["Redb（本地嵌入式 KV）", "嵌入式、WAL", "本地缓存、幂等去重表、熔断器状态、偏移量恢复"],
            ["PostgreSQL", "关系数据库", "消息归档、路由/端点/机构持久化、审计日志、DLQ 持久化、工作流状态"],
        ])
    add_bullet(doc, "可独立启用/禁用任一层")
    add_bullet(doc, "无 PostgreSQL 时自动降级为纯内存模式")

    doc.add_heading("14.2  数据库表结构", level=2)
    add_table(doc,
        ["表名", "说明"],
        [
            ["hsb_messages",              "消息存储（JSONB payload、JSONB headers、BYTEA raw_payload、12 种状态）"],
            ["hsb_routes",                "路由配置（JSONB config）"],
            ["hsb_endpoints",             "端点配置（含版本历史、运行时状态、安全配置）"],
            ["hsb_organizations",         "机构目录"],
            ["hsb_integration_systems",   "系统目录（外键关联机构）"],
            ["hsb_dead_letters",          "死信消息"],
            ["hsb_audit_log",             "审计事件"],
            ["hsb_workflows",             "工作流定义"],
            ["hsb_workflow_instances",     "工作流实例"],
            ["hsb_custom_protocols",      "自定义协议目录"],
            ["hsb_topics",                "Topic 目录"],
            ["hsb_cache",                 "TTL 缓存（UNLOGGED 表）"],
        ])

    # ── 15. 目录与注册 ──
    doc.add_heading("15  目录与注册", level=1)

    doc.add_heading("15.1  机构目录", level=2)
    add_bullet(doc, "机构类型：政府主管部门 / 医院 / 独立法人 / 其他")
    add_bullet(doc, "支持层级结构（parent_organization_id）")
    add_bullet(doc, "机构下的系统管理，删除机构前检查系统引用")

    doc.add_heading("15.2  系统目录", level=2)
    add_bullet(doc, "医疗系统类型：HIS / LIS / RIS / PACS / EMR / HRP / NIS / ORS / 药房 / 省级平台 / 医保 / 第三方 / 其他")
    add_bullet(doc, "每个系统归属一个机构")
    add_bullet(doc, "支持独立 topic_namespace 和 topic_prefix")

    doc.add_heading("15.3  端点目录", level=2)
    add_table(doc,
        ["维度", "说明"],
        [
            ["角色类型",   "Producer（生产者）/ Consumer（消费者）/ Hybrid（混合）"],
            ["生命周期",   "Draft → Active → Disabled → Deprecated → Retired"],
            ["连接配置",   "主机、端口、路径、TLS、超时、连接池、重连间隔、Keepalive"],
            ["认证方式",   "None / Basic / Bearer / ApiKey / OAuth2ClientCredentials / Certificate / SsoToken"],
            ["安全配置",   "TLS 1.2/1.3、双向 TLS、SM3 国密、IP 白名单、凭证掩码、凭证轮换"],
            ["版本管理",   "端点配置变更自动记录版本快照"],
            ["健康监控",   "实时健康状态、延迟、熔断状态、连续失败计数"],
        ])

    # ── 16. 管理与运维 ──
    doc.add_heading("16  管理与运维", level=1)

    doc.add_heading("16.1  RESTful 管理 API", level=2)
    doc.add_paragraph("所有管理操作通过 /api/v1 前缀的 REST API 提供：")
    add_table(doc,
        ["模块", "操作"],
        [
            ["端点",   "CRUD + 状态查询 + 健康检查 + 安全配置 + 版本历史"],
            ["路由",   "CRUD + 启用/禁用"],
            ["消息",   "列表 + 详情 + 重处理"],
            ["死信队列", "列表 + 详情 + 重放 + 删除 + 统计"],
            ["审计",   "事件查询 + 消息链路追踪"],
            ["机构",   "CRUD"],
            ["系统",   "CRUD"],
            ["工作流", "定义 CRUD + 实例启动/暂停/恢复/取消/补偿"],
            ["自定义协议", "CRUD"],
            ["Topic",  "CRUD"],
            ["熔断器", "列表 + 手动重置"],
            ["配置",   "获取 + 更新 + 重载"],
            ["监控",   "健康检查 + 就绪检查 + 系统状态 + 指标"],
        ])

    doc.add_heading("16.2  审计追踪", level=2)
    add_bullet(doc, "所有消息处理环节自动生成审计事件")
    add_bullet(doc, "支持按 message_id 追踪完整处理链路（Span 模型）")
    add_bullet(doc, "审计维度：trace_id / message_id / event_type / source_system / 时间范围")
    add_bullet(doc, "链路状态：InProgress / Completed / PartialSuccess / Failed")

    doc.add_heading("16.3  SSO 集成", level=2)
    add_bullet(doc, "OAuth2 Authorization Code 流程")
    add_bullet(doc, "会话管理（签名 Cookie）")
    add_bullet(doc, "登录 / 登出 / 回调")
    add_bullet(doc, "访问控制：公开路径（/health、/auth/*、入站消息）与受保护路径分离")
    add_bullet(doc, "SSO 启用时，独立 Admin API 端口关闭，统一走主端口认证")

    doc.add_heading("16.4  配置管理", level=2)
    add_bullet(doc, "TOML 格式配置文件")
    add_bullet(doc, "环境变量覆盖（HSB_DATABASE_URL、HSB_MQ_*、HSB_KAFKA_*、HSB_NATS_URLS 等）")
    add_bullet(doc, "生产环境安全校验（拒绝 SSO 默认密码、MQ guest 账户、数据库默认密码）")
    add_bullet(doc, "运行时 API 热更新配置")

    # ── 17. 消息模型 ──
    doc.add_heading("17  消息模型", level=1)

    doc.add_heading("17.1  Canonical Message", level=2)
    doc.add_paragraph("所有协议消息经适配器解析后统一为 Canonical Message：")
    add_table(doc,
        ["字段", "说明"],
        [
            ["ID",           "ULID（时间排序、全局唯一）"],
            ["Topic",        "<domain>.<service>.<action>.<version>（如 medical.order.create.v1）"],
            ["Payload",      "JSON 格式"],
            ["Raw Payload",  "保留原始字节，不可变"],
            ["Headers",      "KV 元数据（必含 trace_id / source / priority）"],
            ["Protocol",     "13 种协议类型标识"],
            ["Status",       "12 种状态（Received → Processing → ... → Completed/Failed）"],
            ["Priority",     "5 级优先级（Low / Normal / High / Urgent / Critical）"],
            ["Metadata",     "医疗领域字段（hospital_id / patient_id / visit_id / order_id 等）"],
            ["Trace ID",     "分布式追踪标识"],
            ["Correlation ID", "请求关联标识"],
        ])

    doc.add_heading("17.2  消息状态机", level=2)
    add_table(doc,
        ["状态", "说明"],
        [
            ["Received",      "消息已接收"],
            ["Processing",    "正在处理"],
            ["Routing",       "正在路由匹配"],
            ["Transforming",  "正在转换"],
            ["Delivering",    "正在投递"],
            ["Delivered",     "已投递"],
            ["Completed",     "处理完成（终态）"],
            ["RetryPending",  "等待重试"],
            ["Retrying",      "正在重试"],
            ["DeadLetter",    "进入死信队列（终态）"],
            ["Failed",        "处理失败（终态）"],
            ["Compensated",   "已补偿（终态）"],
        ])


# ──────────────────────── 主函数 ────────────────────────


def main() -> None:
    print("Nexus HSB 功能列表文档生成工具")
    print("=" * 50)

    doc = Document()
    setup_styles(doc)

    # 封面
    title = doc.add_heading("Nexus HSB 软件功能列表", level=0)
    title.alignment = WD_ALIGN_PARAGRAPH.CENTER
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = p.add_run("医院服务总线 — 医疗信息集成平台")
    run.font.size = Pt(14)
    run.font.color.rgb = RGBColor(0x55, 0x55, 0x55)
    doc.add_paragraph()

    # 第一部分：系统功能界面（含截图）
    print("生成第 1 部分：系统功能界面（含截图）...")
    build_part1_ui(doc)

    # 分隔页
    doc.add_page_break()
    p = doc.add_paragraph()
    p.alignment = WD_ALIGN_PARAGRAPH.CENTER
    run = p.add_run("— 后台核心功能 —")
    run.font.size = Pt(16)
    run.font.color.rgb = RGBColor(0x1A, 0x3C, 0x6E)
    run.bold = True
    doc.add_paragraph()

    # 第二部分：后台核心功能
    print("生成第 2 部分：后台核心功能说明...")
    build_part2_backend(doc)

    # 保存
    doc.save(str(OUTPUT_PATH))
    print(f"\n✓ Word 文档已生成: {OUTPUT_PATH}")


if __name__ == "__main__":
    main()
