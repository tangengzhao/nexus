# Nexus HSB 规范消息模型与无损协议转换设计

## 1. 设计目标

本文档用于细化 HSB 的方案一：规范模型驱动的个性化接口消息定义与协议转换管理。

目标如下：

- 支持每个医院、每个业务线、每个上下游系统自定义接口消息定义
- 支持来源消息定义和目标消息定义不一致的场景
- 支持 HL7、FHIR、DICOM、SOAP、JSON/XML、Webhook、OpenAI 兼容接口等异构协议间转换
- 在转换过程中保留原始报文、扩展字段、未识别字段和协议细节，做到进来不失真
- 让消息建模、接口定义、映射规则、测试样例、发布版本都可视化管理
- 与现有统一 Message、Adapter、TransformerChain、Route 机制兼容

非目标如下：

- 不在第一阶段支持任意复杂脚本编排替代正式工作流
- 不在第一阶段构建通用 BPM 平台
- 不要求所有历史接口立即改造为规范模型模式，允许与旧转换器并存

## 2. 设计原则

### 2.1 规范模型优先

所有协议适配后的业务消息先进入规范消息模型，再根据目标接口定义生成目标报文。避免随着系统数量增长形成 N 对 N 映射爆炸。

### 2.2 原文不可变

来源报文必须原样保存在统一消息对象中，当前已有 [hsb-core/src/message.rs](hsb-core/src/message.rs#L24) 的 raw_payload 字段，可继续作为无损保真基础。

### 2.3 结构化与保真并存

规范模型用于路由、校验、治理和转换；原文、扩展段、未识别字段用于审计、重放和反向重建。

### 2.4 模型和映射分离

消息结构定义、映射模板、字典转换、发布版本必须拆开管理，避免把结构规则硬编码在 Transformer 实现中。

### 2.5 配置先于代码

常见医院项目中的接口差异优先通过配置实现，只有极特殊场景才允许通过脚本扩展或自定义转换器补充。

## 3. 总体架构

### 3.1 核心链路

1. 入站通道接收原始报文
2. 协议适配器解析原始报文，生成统一 Message
3. 基于来源接口定义识别消息 Profile
4. 按来源映射模板将来源结构转换为规范模型
5. 路由层基于规范模型字段进行匹配和分发
6. 按目标映射模板将规范模型转换为目标接口结构
7. 目标协议适配器序列化生成目标报文
8. 持久化转换轨迹、字段映射结果、告警与审计信息

补充说明：当前 `OPENAI` 与 `WEBHOOK` 均作为 HTTP 投递型消费者端点接入规范模型链路；`DATABASE` 已进入端点配置与接口定义模型，但运行态数据库直连 SQL 执行层仍需后续实现。

### 3.2 与现有代码的对应关系

- 统一消息对象： [hsb-core/src/message.rs](hsb-core/src/message.rs#L24)
- 协议解析与序列化入口： [hsb-core/src/adapter.rs](hsb-core/src/adapter.rs#L12)
- 转换链基础： [hsb-core/src/transformer.rs](hsb-core/src/transformer.rs#L48)
- 路由绑定转换器： [hsb-core/src/route.rs](hsb-core/src/route.rs#L13)
- 运行时注册表： [hsb-core/src/engine/registry.rs](hsb-core/src/engine/registry.rs#L11)

建议新增一层 Conversion Profile Runtime，负责把配置中心中的模型、接口定义和映射模板编译成运行时可执行的转换链。

## 4. 核心对象模型

### 4.1 业务域 Business Domain

用于按业务边界组织规范模型，例如：患者、挂号、就诊、医嘱、检验、检查、费用、结算。

关键字段：

- domain_code
- domain_name
- owner_team
- description
- enabled

### 4.2 规范消息模型 Canonical Model

定义业务对象的标准语义结构，是跨系统复用的核心资产。

关键字段：

- model_id
- domain_code
- model_code
- model_name
- version
- event_code
- description
- status: draft/test/published/deprecated
- extensible: 是否允许扩展字段

### 4.3 规范字段 Canonical Field

定义规范模型内每个字段的语义与约束。

关键字段：

- field_path
- field_name
- data_type
- cardinality
- required
- default_value
- enum_set_id
- validation_rule
- source_hint
- target_hint
- extension_allowed

字段路径示例：

- patient.id
- patient.identifiers[].type
- order.items[].code
- visit.encounter_no

### 4.4 接口消息定义 Interface Specification

定义某个系统在某个协议、某个业务事件上的实际接口消息结构。

关键字段：

- spec_id
- system_id
- protocol_type
- direction: producer/consumer/bidirectional
- message_type
- spec_code
- spec_name
- business_domain
- canonical_model_id
- version
- encoding
- delimiter_config
- namespace_config
- root_path
- validation_level
- status

示例：

- HIS_ADT_A01_V2_5_INBOUND
- LIS_ORDER_CREATE_JSON_V1_OUTBOUND
- PACS_REPORT_FHIR_DIAGNOSTICREPORT_V1_OUTBOUND
- LIS_ORDER_CREATED_WEBHOOK_V1_OUTBOUND
- AI_REPORT_SUMMARY_OPENAI_RESPONSES_V1_OUTBOUND
- HIS_PATIENT_LOOKUP_DATABASE_POSTGRESQL_V1_CONSUMER

### 4.5 接口字段 Interface Field

用于描述来源或目标消息的层级结构和约束，支持 HL7 段字段、XML 节点、JSON 路径、FHIR element path。

关键字段：

- field_path
- parent_path
- field_kind: segment/field/component/subcomponent/node/attribute/array/object
- data_type
- required
- repeatable
- position_index
- default_value
- example_value
- enum_set_id
- raw_binding

### 4.6 值集与字典映射 Code Set

用于处理不同系统之间编码不一致问题。

关键字段：

- set_id
- set_code
- set_name
- source_system_scope
- target_system_scope
- mapping_mode: exact/range/pattern/script
- entries

典型场景：

- 性别编码 1/2 转 M/F
- 医嘱状态 0/1/9 转 new/active/cancelled
- 科室编码本地码转主数据码

### 4.7 映射模板 Mapping Template

描述来源接口定义和规范模型、或规范模型和目标接口定义之间的映射关系。

关键字段：

- template_id
- template_code
- template_name
- mapping_direction: source_to_canonical/canonical_to_target
- source_spec_id
- target_spec_id
- canonical_model_id
- version
- status
- priority
- script_policy

### 4.8 映射规则 Mapping Rule

映射模板下的最小执行单元。

关键字段：

- rule_id
- template_id
- source_path
- target_path
- rule_type: copy/constant/dictionary/expression/conditional/split/merge/loop/object_construct
- required
- default_value
- expression
- dictionary_set_id
- fallback_policy
- validation_policy
- order_no

### 4.9 无损保真策略 Lossless Retention Policy

定义哪些原文、扩展字段和格式细节必须保留。

关键字段：

- policy_id
- policy_name
- keep_raw_payload
- keep_original_headers
- keep_unmapped_fields
- keep_unknown_segments
- keep_protocol_metadata
- roundtrip_required
- replay_from_raw_preferred

### 4.10 转换发布包 Conversion Package

把规范模型版本、接口定义版本、映射模板版本、值集版本打成一个可发布单元，由路由或端点引用。

关键字段：

- package_id
- package_code
- package_name
- business_domain
- source_spec_id
- canonical_model_id
- target_spec_id
- source_mapping_template_id
- target_mapping_template_id
- lossless_policy_id
- package_version
- release_status
- effective_from
- effective_to

### 4.11 测试样例 Conversion Test Case

支持样例驱动验证。

关键字段：

- case_id
- package_id
- case_name
- source_raw_message
- expected_canonical_json
- expected_target_raw_message
- assertions
- tags

## 5. 无损转换设计

### 5.1 保真存储分层

单条消息转换过程中至少保留以下四层数据：

1. 原始入站报文
2. 解析后的来源结构化视图
3. 规范模型视图
4. 目标结构化视图和目标原始报文

### 5.2 未映射字段保留策略

未被规范模型吸收的字段不得直接丢弃，建议保存到以下区域：

- message.metadata.custom 中的扩展区
- 专门的 unmapped_fields JSONB 字段
- protocol_artifacts JSONB，保存编码、分隔符、命名空间、段顺序等协议细节

### 5.3 回放与重建策略

必须支持两类重放：

- 基于原始报文重放
- 基于规范模型和目标模板重建目标报文重放

第一类用于严格保真回放，第二类用于配置修复后的重新生成。

## 6. 配置中心信息架构

### 6.1 一级菜单建议

集成设计下新增消息建模专区，替代单一“转换器中心”概念。

### 6.2 二级菜单建议

- 消息建模
  - 业务域管理
  - 规范模型中心
  - 接口消息定义
  - 字段字典中心
  - 映射模板中心
  - 转换发布中心
  - 转换测试台

### 6.3 页面说明

#### 6.3.1 规范模型中心

用于维护业务标准模型及字段。

页面能力：

- 模型列表、搜索、复制新版本
- 字段树编辑
- 必填与约束配置
- 模型差异比较
- 模型引用分析

#### 6.3.2 接口消息定义

用于维护某个系统真实接口结构。

页面能力：

- 生产者接口定义列表
- 消费者接口定义列表
- 协议结构树编辑器
- 消息样例导入自动生成初版结构
- 字段路径预览
- 版本历史与回滚

#### 6.3.3 映射模板中心

用于配置字段映射和规则。

页面能力：

- 左侧来源结构、中间规范模型、右侧目标结构的三栏映射视图
- 支持拖拽映射和规则表格编辑双模式
- 支持字典映射、条件映射、表达式映射、数组迭代、对象构造
- 支持映射缺口分析和必填字段覆盖率统计

#### 6.3.4 转换测试台

用于验证来源报文到规范模型、规范模型到目标报文的完整转换。

页面能力：

- 粘贴原始报文
- 展示来源结构化视图
- 展示规范模型 JSON
- 展示目标结构和目标原文
- 展示字段级转换轨迹
- 展示校验错误、保真告警、丢字段告警

#### 6.3.5 转换发布中心

用于将多个对象打包发布。

页面能力：

- 选择模型版本、接口定义版本、映射模板版本
- 生成 Conversion Package
- 绑定生效时间
- 查看受影响路由
- 灰度发布与回滚

## 7. 关键配置流程

### 7.1 新建个性化来源接口

1. 选择业务域
2. 选择或新建规范模型
3. 新建来源接口定义
4. 导入样例报文自动生成结构树
5. 补齐字段说明、类型、重复性、编码规则
6. 创建 source_to_canonical 映射模板
7. 执行测试样例
8. 发布版本

### 7.2 新建目标接口转换

1. 选择规范模型
2. 新建目标接口定义
3. 配置 canonical_to_target 映射模板
4. 绑定值集映射
5. 校验目标必填字段覆盖率
6. 执行端到端测试
7. 发布转换包

### 7.3 路由绑定方式

Route 不再直接只绑定通用 transformer_ids，而是优先绑定转换发布包。运行时由发布包展开为：

- 来源接口识别器
- source_to_canonical 转换链
- canonical_to_target 转换链
- 序列化器

为兼容当前实现，运行时可把转换发布包编译成具名 transformer 集合，再挂接到现有 route.transformer_ids。

## 8. 运行时执行模型

### 8.1 新增运行时组件

建议新增以下抽象：

- CanonicalModelRegistry
- InterfaceSpecRegistry
- MappingTemplateRegistry
- ConversionPackageRegistry
- ConversionExecutionService
- ConversionTraceStore

### 8.2 执行步骤

1. 根据来源协议和系统识别 Interface Spec
2. 调用协议适配器 parse
3. 生成来源结构化视图
4. 执行 source_to_canonical 模板，得到 canonical payload
5. 执行 canonical validation
6. 路由匹配
7. 按目标端点选择 canonical_to_target 模板
8. 执行模板，得到 target payload
9. 调用目标协议适配器 serialize
10. 持久化 trace、告警、映射结果

### 8.3 字段级追踪

每条映射规则执行后生成一条 trace 明细，建议包含：

- message_id
- package_version
- template_id
- rule_id
- source_path
- target_path
- source_value_snapshot
- target_value_snapshot
- execution_status
- warning_code
- elapsed_ms

## 9. 持久化设计

建议在 PostgreSQL 中新增如下表：

- hsb_business_domains
- hsb_canonical_models
- hsb_canonical_fields
- hsb_interface_specs
- hsb_interface_fields
- hsb_code_sets
- hsb_code_set_entries
- hsb_mapping_templates
- hsb_mapping_rules
- hsb_lossless_policies
- hsb_conversion_packages
- hsb_conversion_test_cases
- hsb_conversion_traces

### 9.1 关键表说明

#### hsb_canonical_models

- id
- domain_code
- model_code
- model_name
- version
- event_code
- description
- status
- schema_json
- created_at
- updated_at

#### hsb_interface_specs

- id
- system_id
- protocol_type
- direction
- message_type
- spec_code
- spec_name
- canonical_model_id
- version
- encoding
- structure_json
- validation_json
- status
- created_at
- updated_at

#### hsb_mapping_templates

- id
- template_code
- mapping_direction
- source_spec_id
- target_spec_id
- canonical_model_id
- version
- status
- config_json
- created_at
- updated_at

#### hsb_conversion_packages

- id
- package_code
- source_spec_id
- canonical_model_id
- target_spec_id
- source_template_id
- target_template_id
- lossless_policy_id
- package_version
- release_status
- effective_from
- effective_to
- created_at
- updated_at

#### hsb_conversion_traces

- id
- message_id
- package_id
- template_id
- rule_id
- source_path
- target_path
- source_value_json
- target_value_json
- execution_status
- warning_json
- elapsed_ms
- created_at

## 10. 管理 API 设计

建议在现有 /api/v1 下补齐资源：

- /canonical-domains
- /canonical-models
- /canonical-models/{id}/fields
- /interface-specs
- /interface-specs/{id}/fields
- /code-sets
- /mapping-templates
- /mapping-templates/{id}/rules
- /conversion-packages
- /conversion-packages/{id}/publish
- /conversion-packages/{id}/test
- /conversion-test-cases
- /conversion-traces

推荐增加专项操作接口：

- POST /interface-specs/import-sample
- POST /mapping-templates/{id}/coverage-analysis
- POST /conversion-packages/{id}/compile
- POST /conversion-packages/{id}/simulate
- POST /conversion-packages/{id}/rollback

## 11. 权限与审批建议

建议新增细粒度权限：

- 规范模型查看/编辑/发布
- 接口定义查看/编辑/发布
- 映射模板查看/编辑/测试/发布
- 转换发布包创建/发布/回滚
- 转换测试台执行
- 转换轨迹查看

涉及生产生效的操作建议纳入审批：

- 发布新模型版本
- 发布新映射模板版本
- 切换转换包生效版本
- 回滚转换包

## 12. 分阶段落地建议

### 第一阶段

- 规范模型中心
- 接口消息定义
- 映射模板中心
- 转换测试台
- Conversion Package 基础发布

### 第二阶段

- 字典映射与主数据联动
- 字段级 trace 存储和 UI 展示
- 覆盖率分析与影响分析
- 路由直接引用转换发布包

### 第三阶段

- 样例自动逆向建模增强
- 协议 roundtrip 校验
- 灰度发布和按租户生效
- 复杂数组、片段和组合结构图形化映射

## 13. 对当前代码改造建议

第一批代码改造建议如下：

1. 在 hsb-core 增加规范模型、接口定义、映射模板、转换发布包的领域对象与 store trait
2. 在 hsb-core 的 PostgreSQL store 中新增对应表和实现
3. 在 hsb-admin 新增模型中心、接口定义、映射模板和转换发布包 CRUD API
4. 在 hsb-server 的 UI 中新增消息建模专区
5. 在运行时新增 ConversionExecutionService，把发布包编译为现有 TransformerChain 可执行对象
6. 在消息持久化中补充 conversion_trace 持久化

这一路径能最大程度复用当前 Message、Adapter、Transformer 和 Route 能力，同时把个性化接口消息定义从“单个转换器配置”升级为“完整消息建模体系”。