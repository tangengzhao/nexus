//! 工作流和编排定义
//!
//! 支持多步骤工作流、Saga 模式和补偿事务。

use crate::message::Message;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hsb_common::{HsbError, HsbResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use ulid::Ulid;

/// 工作流定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// 工作流 ID
    pub id: String,

    /// 工作流名称
    pub name: String,

    /// 工作流描述
    pub description: Option<String>,

    /// 工作流版本
    pub version: u32,

    /// 步骤列表
    pub steps: Vec<WorkflowStep>,

    /// 全局超时
    pub timeout: Duration,

    /// 补偿策略
    pub compensation: Option<CompensationPolicy>,

    /// 是否启用
    pub enabled: bool,

    /// 工作流选项
    pub options: WorkflowOptions,
}

/// 工作流步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    /// 步骤 ID
    pub id: String,

    /// 步骤名称
    pub name: String,

    /// 步骤类型
    pub step_type: StepType,

    /// 步骤配置
    pub config: StepConfig,

    /// 重试策略
    pub retry: Option<RetryPolicy>,

    /// 超时
    pub timeout: Option<Duration>,

    /// 条件（满足条件才执行）
    pub condition: Option<String>,

    /// 补偿操作
    pub compensation_step: Option<Box<WorkflowStep>>,

    /// 下一步（条件跳转）
    pub next_steps: Vec<NextStep>,
}

/// 步骤类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepType {
    /// 发送消息到端点
    Send {
        endpoint_id: String,
        transformer_ids: Vec<String>,
    },

    /// 接收响应
    Receive { timeout_ms: u64 },

    /// 转换消息
    Transform { transformer_ids: Vec<String> },

    /// 并行执行
    Parallel {
        branches: Vec<Vec<WorkflowStep>>,
        join_mode: JoinMode,
    },

    /// 条件分支
    Choice {
        branches: Vec<ChoiceBranch>,
        default_branch: Option<Box<WorkflowStep>>,
    },

    /// 脚本执行
    Script {
        language: ScriptLanguage,
        code: String,
    },

    /// 等待
    Wait { duration_ms: u64 },

    /// 子工作流
    SubWorkflow { workflow_id: String },

    /// 日志
    Log { level: String, message: String },
}

/// 步骤配置
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StepConfig {
    /// 是否异步执行
    pub async_execution: bool,

    /// 是否可跳过
    pub skippable: bool,

    /// 输入映射
    pub input_mapping: HashMap<String, String>,

    /// 输出映射
    pub output_mapping: HashMap<String, String>,

    /// 自定义属性
    pub properties: HashMap<String, serde_json::Value>,
}

/// 并行执行的合并模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JoinMode {
    /// 等待所有分支完成
    All,
    /// 等待任一分支完成
    Any,
    /// 等待 N 个分支完成
    N(usize),
}

/// 条件分支
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceBranch {
    /// 条件表达式
    pub condition: String,
    /// 分支步骤
    pub steps: Vec<WorkflowStep>,
}

/// 脚本语言
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScriptLanguage {
    /// JavaScript（使用 Deno/QuickJS）
    JavaScript,
    /// Lua
    Lua,
    /// 表达式语言
    Expression,
}

/// 下一步定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextStep {
    /// 下一步 ID
    pub step_id: String,
    /// 条件（可选）
    pub condition: Option<String>,
}

/// 重试策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryPolicy {
    /// 最大重试次数
    pub max_attempts: u32,
    /// 初始延迟（毫秒）
    pub initial_delay_ms: u64,
    /// 最大延迟（毫秒）
    pub max_delay_ms: u64,
    /// 退避乘数
    pub multiplier: f64,
    /// 可重试的错误类型
    pub retryable_errors: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            multiplier: 2.0,
            retryable_errors: Vec::new(),
        }
    }
}

/// 补偿策略
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompensationPolicy {
    /// 补偿模式
    pub mode: CompensationMode,
    /// 补偿超时
    pub timeout: Duration,
    /// 是否在补偿失败时继续
    pub continue_on_failure: bool,
}

/// 补偿模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationMode {
    /// 按顺序反向补偿
    Sequential,
    /// 并行补偿
    Parallel,
    /// 手动补偿
    Manual,
}

/// 工作流选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowOptions {
    /// 是否持久化状态
    pub persist_state: bool,
    /// 是否支持暂停/恢复
    pub pausable: bool,
    /// 最大并发实例数
    pub max_concurrent_instances: Option<u32>,
    /// 实例超时（秒）
    pub instance_timeout_secs: u64,
}

impl Default for WorkflowOptions {
    fn default() -> Self {
        Self {
            persist_state: true,
            pausable: true,
            max_concurrent_instances: None,
            instance_timeout_secs: 3600,
        }
    }
}

// ============ 工作流实例 ============

/// 工作流实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowInstance {
    /// 实例 ID
    pub id: Ulid,

    /// 工作流 ID
    pub workflow_id: String,

    /// 工作流版本
    pub workflow_version: u32,

    /// 实例状态
    pub status: WorkflowStatus,

    /// 当前步骤 ID
    pub current_step_id: Option<String>,

    /// 上下文数据
    pub context: WorkflowContext,

    /// 步骤执行历史
    pub step_history: Vec<StepExecution>,

    /// 创建时间
    pub created_at: DateTime<Utc>,

    /// 更新时间
    pub updated_at: DateTime<Utc>,

    /// 完成时间
    pub completed_at: Option<DateTime<Utc>>,

    /// 错误信息
    pub error: Option<String>,
}

/// 工作流状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WorkflowStatus {
    /// 待执行
    Pending,
    /// 执行中
    Running,
    /// 已暂停
    Paused,
    /// 等待外部事件
    Waiting,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 补偿中
    Compensating,
    /// 已补偿
    Compensated,
    /// 已取消
    Cancelled,
}

impl WorkflowStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Compensated | Self::Cancelled
        )
    }

    pub fn is_active(&self) -> bool {
        matches!(self, Self::Running | Self::Compensating)
    }
}

/// 工作流上下文
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowContext {
    /// 输入消息
    pub input_message: Option<Message>,
    /// 输出消息
    pub output_message: Option<Message>,
    /// 变量存储
    pub variables: HashMap<String, serde_json::Value>,
    /// 步骤输出
    pub step_outputs: HashMap<String, serde_json::Value>,
}

impl WorkflowContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_input(mut self, msg: Message) -> Self {
        self.input_message = Some(msg);
        self
    }

    pub fn set_variable(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.variables.insert(key.into(), value);
    }

    pub fn get_variable(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }

    pub fn set_step_output(&mut self, step_id: impl Into<String>, output: serde_json::Value) {
        self.step_outputs.insert(step_id.into(), output);
    }
}

/// 步骤执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepExecution {
    /// 步骤 ID
    pub step_id: String,
    /// 执行状态
    pub status: StepExecutionStatus,
    /// 开始时间
    pub started_at: DateTime<Utc>,
    /// 结束时间
    pub ended_at: Option<DateTime<Utc>>,
    /// 重试次数
    pub retry_count: u32,
    /// 输入数据
    pub input: Option<serde_json::Value>,
    /// 输出数据
    pub output: Option<serde_json::Value>,
    /// 错误信息
    pub error: Option<String>,
}

/// 步骤执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StepExecutionStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
    Compensated,
}

// ============ 工作流执行器 trait ============

/// 工作流执行器
#[async_trait]
pub trait WorkflowExecutor: Send + Sync {
    /// 启动工作流实例
    async fn start(&self, workflow: &Workflow, input: Message) -> HsbResult<WorkflowInstance>;

    /// 恢复工作流实例
    async fn resume(&self, instance_id: Ulid) -> HsbResult<WorkflowInstance>;

    /// 暂停工作流实例
    async fn pause(&self, instance_id: Ulid) -> HsbResult<()>;

    /// 取消工作流实例
    async fn cancel(&self, instance_id: Ulid) -> HsbResult<()>;

    /// 获取工作流实例状态
    async fn get_instance(&self, instance_id: Ulid) -> HsbResult<Option<WorkflowInstance>>;

    /// 列出所有工作流实例
    async fn list_instances(&self) -> HsbResult<Vec<WorkflowInstance>>;

    /// 触发补偿
    async fn compensate(&self, instance_id: Ulid) -> HsbResult<()>;
}

#[async_trait]
pub trait WorkflowStepHandler: Send + Sync {
    async fn execute(
        &self,
        step: &WorkflowStep,
        context: &mut WorkflowContext,
    ) -> HsbResult<Option<serde_json::Value>>;

    async fn compensate(
        &self,
        _step: &WorkflowStep,
        _context: &mut WorkflowContext,
    ) -> HsbResult<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct InMemoryWorkflowExecutor {
    workflows: Arc<RwLock<HashMap<String, Workflow>>>,
    instances: Arc<RwLock<HashMap<Ulid, WorkflowInstance>>>,
    handler: Arc<dyn WorkflowStepHandler>,
}

impl InMemoryWorkflowExecutor {
    pub fn new(handler: Arc<dyn WorkflowStepHandler>) -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            handler,
        }
    }

    async fn execute_workflow_background(
        &self,
        workflow: Workflow,
        instance_id: Ulid,
    ) -> HsbResult<()> {
        if !workflow.enabled {
            return Err(HsbError::WorkflowError {
                workflow_id: workflow.id.clone(),
                step: String::new(),
                reason: "workflow is disabled".to_string(),
            });
        }

        self.update_instance(instance_id, |instance| {
            if matches!(instance.status, WorkflowStatus::Pending) {
                instance.status = WorkflowStatus::Running;
                instance.updated_at = Utc::now();
            }
        })
        .await?;

        loop {
            let mut instance = self.load_instance(instance_id).await?;

            if matches!(
                instance.status,
                WorkflowStatus::Cancelled | WorkflowStatus::Paused
            ) {
                return Ok(());
            }

            let Some(step_id) = instance
                .current_step_id
                .clone()
                .or_else(|| workflow.first_step().map(|step| step.id.clone()))
            else {
                instance.status = WorkflowStatus::Completed;
                instance.current_step_id = None;
                instance.completed_at = Some(Utc::now());
                instance.updated_at = Utc::now();
                self.store_instance(instance).await;
                return Ok(());
            };

            let step =
                workflow
                    .get_step(&step_id)
                    .cloned()
                    .ok_or_else(|| HsbError::WorkflowError {
                        workflow_id: workflow.id.clone(),
                        step: step_id.clone(),
                        reason: "step not found".to_string(),
                    })?;

            instance.current_step_id = Some(step.id.clone());
            let mut execution = StepExecution {
                step_id: step.id.clone(),
                status: StepExecutionStatus::Running,
                started_at: Utc::now(),
                ended_at: None,
                retry_count: 0,
                input: instance
                    .context
                    .input_message
                    .as_ref()
                    .and_then(|message| serde_json::to_value(message).ok()),
                output: None,
                error: None,
            };

            let step_result = self.execute_step(&step, &mut instance.context).await;

            let latest = self.load_instance(instance_id).await?;
            if matches!(latest.status, WorkflowStatus::Cancelled) {
                return Ok(());
            }

            match step_result {
                Ok(output) => {
                    execution.status = StepExecutionStatus::Completed;
                    execution.ended_at = Some(Utc::now());
                    execution.output = output.clone();
                    if let Some(output) = output {
                        instance.context.set_step_output(&step.id, output);
                    }
                    instance.step_history.push(execution);
                    instance.current_step_id = resolve_next_step(&workflow, &step);
                    instance.updated_at = Utc::now();

                    if latest.status == WorkflowStatus::Paused {
                        instance.status = WorkflowStatus::Paused;
                    } else if instance.current_step_id.is_none() {
                        instance.status = WorkflowStatus::Completed;
                        instance.completed_at = Some(Utc::now());
                    } else {
                        instance.status = WorkflowStatus::Running;
                    }

                    self.store_instance(instance.clone()).await;

                    if matches!(
                        instance.status,
                        WorkflowStatus::Paused | WorkflowStatus::Completed
                    ) {
                        return Ok(());
                    }
                }
                Err(error) => {
                    execution.status = StepExecutionStatus::Failed;
                    execution.ended_at = Some(Utc::now());
                    execution.error = Some(error.to_string());
                    instance.step_history.push(execution);
                    instance.status = WorkflowStatus::Failed;
                    instance.error = Some(error.to_string());
                    instance.updated_at = Utc::now();

                    if workflow.compensation.is_some() {
                        self.compensate_instance(&workflow, &mut instance).await?;
                    }

                    self.store_instance(instance).await;
                    return Ok(());
                }
            }
        }
    }

    async fn execute_step(
        &self,
        step: &WorkflowStep,
        context: &mut WorkflowContext,
    ) -> HsbResult<Option<serde_json::Value>> {
        match &step.step_type {
            StepType::Wait { duration_ms } => {
                tokio::time::sleep(Duration::from_millis(*duration_ms)).await;
                Ok(Some(serde_json::json!({ "waited_ms": duration_ms })))
            }
            StepType::Log { level, message } => Ok(Some(serde_json::json!({
                "level": level,
                "message": message,
            }))),
            _ => self.handler.execute(step, context).await,
        }
    }

    async fn compensate_instance(
        &self,
        workflow: &Workflow,
        instance: &mut WorkflowInstance,
    ) -> HsbResult<()> {
        instance.status = WorkflowStatus::Compensating;
        instance.updated_at = Utc::now();

        for execution in instance.step_history.clone().into_iter().rev() {
            if execution.status != StepExecutionStatus::Completed {
                continue;
            }

            let Some(step) = workflow.get_step(&execution.step_id) else {
                continue;
            };
            let Some(compensation_step) = step.compensation_step.as_ref() else {
                continue;
            };

            self.handler
                .compensate(compensation_step, &mut instance.context)
                .await?;

            instance.step_history.push(StepExecution {
                step_id: compensation_step.id.clone(),
                status: StepExecutionStatus::Compensated,
                started_at: Utc::now(),
                ended_at: Some(Utc::now()),
                retry_count: 0,
                input: None,
                output: None,
                error: None,
            });
        }

        instance.status = WorkflowStatus::Compensated;
        instance.current_step_id = None;
        instance.completed_at = Some(Utc::now());
        instance.updated_at = Utc::now();
        Ok(())
    }

    async fn load_instance(&self, instance_id: Ulid) -> HsbResult<WorkflowInstance> {
        self.instances
            .read()
            .await
            .get(&instance_id)
            .cloned()
            .ok_or_else(|| HsbError::NotFound {
                entity: "WorkflowInstance".to_string(),
                id: instance_id.to_string(),
            })
    }

    async fn update_instance<F>(&self, instance_id: Ulid, updater: F) -> HsbResult<()>
    where
        F: FnOnce(&mut WorkflowInstance),
    {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&instance_id)
            .ok_or_else(|| HsbError::NotFound {
                entity: "WorkflowInstance".to_string(),
                id: instance_id.to_string(),
            })?;
        updater(instance);
        Ok(())
    }

    async fn store_instance(&self, instance: WorkflowInstance) {
        self.instances.write().await.insert(instance.id, instance);
    }

    fn spawn_execution(&self, workflow: Workflow, instance_id: Ulid) {
        let executor = self.clone();
        tokio::spawn(async move {
            if let Err(error) = executor
                .execute_workflow_background(workflow, instance_id)
                .await
            {
                let _ = executor
                    .update_instance(instance_id, |instance| {
                        instance.status = WorkflowStatus::Failed;
                        instance.error = Some(error.to_string());
                        instance.updated_at = Utc::now();
                    })
                    .await;
            }
        });
    }
}

#[async_trait]
impl WorkflowExecutor for InMemoryWorkflowExecutor {
    async fn start(&self, workflow: &Workflow, input: Message) -> HsbResult<WorkflowInstance> {
        self.workflows
            .write()
            .await
            .insert(workflow.id.clone(), workflow.clone());

        let now = Utc::now();
        let instance = WorkflowInstance {
            id: Ulid::new(),
            workflow_id: workflow.id.clone(),
            workflow_version: workflow.version,
            status: WorkflowStatus::Pending,
            current_step_id: workflow.first_step().map(|step| step.id.clone()),
            context: WorkflowContext::new().with_input(input),
            step_history: Vec::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            error: None,
        };

        self.instances
            .write()
            .await
            .insert(instance.id, instance.clone());
        self.spawn_execution(workflow.clone(), instance.id);
        Ok(instance)
    }

    async fn resume(&self, instance_id: Ulid) -> HsbResult<WorkflowInstance> {
        let workflow_id = {
            let instances = self.instances.read().await;
            instances
                .get(&instance_id)
                .map(|instance| instance.workflow_id.clone())
                .ok_or_else(|| HsbError::NotFound {
                    entity: "WorkflowInstance".to_string(),
                    id: instance_id.to_string(),
                })?
        };
        let workflow = {
            let workflows = self.workflows.read().await;
            workflows
                .get(&workflow_id)
                .cloned()
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Workflow".to_string(),
                    id: workflow_id.clone(),
                })?
        };

        self.update_instance(instance_id, |instance| {
            instance.status = WorkflowStatus::Pending;
            instance.updated_at = Utc::now();
            if instance.current_step_id.is_none() {
                instance.current_step_id = workflow.first_step().map(|step| step.id.clone());
            }
        })
        .await?;

        let instance = self.load_instance(instance_id).await?;
        self.spawn_execution(workflow, instance_id);
        Ok(instance)
    }

    async fn pause(&self, instance_id: Ulid) -> HsbResult<()> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&instance_id)
            .ok_or_else(|| HsbError::NotFound {
                entity: "WorkflowInstance".to_string(),
                id: instance_id.to_string(),
            })?;
        instance.status = WorkflowStatus::Paused;
        instance.updated_at = Utc::now();
        Ok(())
    }

    async fn cancel(&self, instance_id: Ulid) -> HsbResult<()> {
        let mut instances = self.instances.write().await;
        let instance = instances
            .get_mut(&instance_id)
            .ok_or_else(|| HsbError::NotFound {
                entity: "WorkflowInstance".to_string(),
                id: instance_id.to_string(),
            })?;
        instance.status = WorkflowStatus::Cancelled;
        instance.current_step_id = None;
        instance.completed_at = Some(Utc::now());
        instance.updated_at = Utc::now();
        Ok(())
    }

    async fn get_instance(&self, instance_id: Ulid) -> HsbResult<Option<WorkflowInstance>> {
        Ok(self.instances.read().await.get(&instance_id).cloned())
    }

    async fn list_instances(&self) -> HsbResult<Vec<WorkflowInstance>> {
        let mut instances: Vec<_> = self.instances.read().await.values().cloned().collect();
        instances.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(instances)
    }

    async fn compensate(&self, instance_id: Ulid) -> HsbResult<()> {
        let workflow_id = {
            let instances = self.instances.read().await;
            instances
                .get(&instance_id)
                .map(|instance| instance.workflow_id.clone())
                .ok_or_else(|| HsbError::NotFound {
                    entity: "WorkflowInstance".to_string(),
                    id: instance_id.to_string(),
                })?
        };
        let workflow = {
            let workflows = self.workflows.read().await;
            workflows
                .get(&workflow_id)
                .cloned()
                .ok_or_else(|| HsbError::NotFound {
                    entity: "Workflow".to_string(),
                    id: workflow_id.clone(),
                })?
        };

        let mut instance = {
            let instances = self.instances.read().await;
            instances
                .get(&instance_id)
                .cloned()
                .ok_or_else(|| HsbError::NotFound {
                    entity: "WorkflowInstance".to_string(),
                    id: instance_id.to_string(),
                })?
        };
        self.compensate_instance(&workflow, &mut instance).await?;
        self.instances.write().await.insert(instance.id, instance);
        Ok(())
    }
}

fn resolve_next_step(workflow: &Workflow, step: &WorkflowStep) -> Option<String> {
    if let Some(next) = step.next_steps.iter().find(|next| next.condition.is_none()) {
        return Some(next.step_id.clone());
    }

    workflow
        .steps
        .iter()
        .position(|candidate| candidate.id == step.id)
        .and_then(|index| workflow.steps.get(index + 1))
        .map(|step| step.id.clone())
}

impl Workflow {
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            version: 1,
            steps: Vec::new(),
            timeout: Duration::from_secs(3600),
            compensation: None,
            enabled: true,
            options: WorkflowOptions::default(),
        }
    }

    pub fn with_step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    pub fn with_compensation(mut self, policy: CompensationPolicy) -> Self {
        self.compensation = Some(policy);
        self
    }

    pub fn get_step(&self, step_id: &str) -> Option<&WorkflowStep> {
        self.steps.iter().find(|s| s.id == step_id)
    }

    pub fn first_step(&self) -> Option<&WorkflowStep> {
        self.steps.first()
    }
}

impl WorkflowStep {
    pub fn send(
        id: impl Into<String>,
        name: impl Into<String>,
        endpoint_id: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            step_type: StepType::Send {
                endpoint_id: endpoint_id.into(),
                transformer_ids: Vec::new(),
            },
            config: StepConfig::default(),
            retry: Some(RetryPolicy::default()),
            timeout: Some(Duration::from_secs(30)),
            condition: None,
            compensation_step: None,
            next_steps: Vec::new(),
        }
    }

    pub fn transform(
        id: impl Into<String>,
        name: impl Into<String>,
        transformer_ids: Vec<String>,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            step_type: StepType::Transform { transformer_ids },
            config: StepConfig::default(),
            retry: None,
            timeout: Some(Duration::from_secs(10)),
            condition: None,
            compensation_step: None,
            next_steps: Vec::new(),
        }
    }

    pub fn wait(id: impl Into<String>, duration: Duration) -> Self {
        Self {
            id: id.into(),
            name: "Wait".to_string(),
            step_type: StepType::Wait {
                duration_ms: duration.as_millis() as u64,
            },
            config: StepConfig::default(),
            retry: None,
            timeout: None,
            condition: None,
            compensation_step: None,
            next_steps: Vec::new(),
        }
    }

    pub fn log(id: impl Into<String>, name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            step_type: StepType::Log {
                level: "info".to_string(),
                message: message.into(),
            },
            config: StepConfig::default(),
            retry: None,
            timeout: None,
            condition: None,
            compensation_step: None,
            next_steps: Vec::new(),
        }
    }

    pub fn with_compensation(mut self, step: WorkflowStep) -> Self {
        self.compensation_step = Some(Box::new(step));
        self
    }

    pub fn with_next(mut self, step_id: impl Into<String>) -> Self {
        self.next_steps.push(NextStep {
            step_id: step_id.into(),
            condition: None,
        });
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MessageBuilder;
    use hsb_common::ProtocolType;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::time::{Duration as TokioDuration, sleep};

    #[derive(Default)]
    struct MockWorkflowHandler {
        executed: Arc<Mutex<Vec<String>>>,
        compensated: Arc<Mutex<Vec<String>>>,
        fail_step: Option<String>,
    }

    #[async_trait]
    impl WorkflowStepHandler for MockWorkflowHandler {
        async fn execute(
            &self,
            step: &WorkflowStep,
            _context: &mut WorkflowContext,
        ) -> HsbResult<Option<serde_json::Value>> {
            self.executed.lock().await.push(step.id.clone());
            if self.fail_step.as_deref() == Some(step.id.as_str()) {
                return Err(HsbError::WorkflowError {
                    workflow_id: "wf_test".to_string(),
                    step: step.id.clone(),
                    reason: "forced failure".to_string(),
                });
            }
            Ok(Some(serde_json::json!({ "step_id": step.id })))
        }

        async fn compensate(
            &self,
            step: &WorkflowStep,
            _context: &mut WorkflowContext,
        ) -> HsbResult<()> {
            self.compensated.lock().await.push(step.id.clone());
            Ok(())
        }
    }

    #[test]
    fn test_workflow_creation() {
        let workflow = Workflow::new("wf_001", "Patient Registration")
            .with_step(WorkflowStep::send("step_1", "Send to HIS", "HIS"))
            .with_step(WorkflowStep::send("step_2", "Send to LIS", "LIS"));

        assert_eq!(workflow.steps.len(), 2);
        assert_eq!(workflow.first_step().map(|s| s.id.as_str()), Some("step_1"));
    }

    #[tokio::test]
    async fn in_memory_executor_runs_workflow_steps_in_order() {
        let handler = Arc::new(MockWorkflowHandler::default());
        let executor = InMemoryWorkflowExecutor::new(handler.clone());
        let workflow = Workflow::new("wf_test", "Test Workflow")
            .with_step(WorkflowStep::send("step_1", "Send to HIS", "HIS"))
            .with_step(WorkflowStep::wait("step_2", Duration::from_millis(1)))
            .with_step(WorkflowStep::send("step_3", "Send to LIS", "LIS"));

        let input = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .raw_payload(br#"{"patient_id":"P001"}"#.to_vec())
            .build()
            .expect("message should build");

        let instance = executor
            .start(&workflow, input)
            .await
            .expect("workflow should start");
        let instance = wait_for_terminal_instance(&executor, instance.id).await;

        assert_eq!(instance.status, WorkflowStatus::Completed);
        assert_eq!(instance.step_history.len(), 3);
        let executed = handler.executed.lock().await.clone();
        assert_eq!(executed, vec!["step_1".to_string(), "step_3".to_string()]);
    }

    #[tokio::test]
    async fn in_memory_executor_compensates_completed_steps_on_failure() {
        let handler = Arc::new(MockWorkflowHandler {
            fail_step: Some("step_2".to_string()),
            ..Default::default()
        });
        let executor = InMemoryWorkflowExecutor::new(handler.clone());
        let workflow = Workflow::new("wf_test", "Compensating Workflow")
            .with_compensation(CompensationPolicy {
                mode: CompensationMode::Sequential,
                timeout: Duration::from_secs(5),
                continue_on_failure: false,
            })
            .with_step(
                WorkflowStep::send("step_1", "Send to HIS", "HIS")
                    .with_compensation(WorkflowStep::log("comp_1", "Undo HIS", "revert")),
            )
            .with_step(WorkflowStep::send("step_2", "Send to LIS", "LIS"));

        let input = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .raw_payload(br#"{"patient_id":"P001"}"#.to_vec())
            .build()
            .expect("message should build");

        let instance = executor
            .start(&workflow, input)
            .await
            .expect("workflow should start");
        let instance = wait_for_terminal_instance(&executor, instance.id).await;

        assert_eq!(instance.status, WorkflowStatus::Compensated);
        let compensated = handler.compensated.lock().await.clone();
        assert_eq!(compensated, vec!["comp_1".to_string()]);
    }

    #[tokio::test]
    async fn in_memory_executor_supports_pause_and_resume() {
        let handler = Arc::new(MockWorkflowHandler::default());
        let executor = InMemoryWorkflowExecutor::new(handler.clone());
        let workflow = Workflow::new("wf_pause", "Pause Workflow")
            .with_step(WorkflowStep::wait("step_1", Duration::from_millis(25)))
            .with_step(WorkflowStep::send("step_2", "Send to LIS", "LIS"));

        let input = MessageBuilder::new()
            .source_system("HIS")
            .protocol(ProtocolType::Http)
            .raw_payload(br#"{"patient_id":"P001"}"#.to_vec())
            .build()
            .expect("message should build");

        let instance = executor
            .start(&workflow, input)
            .await
            .expect("workflow should start");
        executor
            .pause(instance.id)
            .await
            .expect("pause should succeed");
        sleep(TokioDuration::from_millis(30)).await;

        let paused = executor
            .get_instance(instance.id)
            .await
            .expect("instance query should succeed")
            .expect("instance should exist");
        assert_eq!(paused.status, WorkflowStatus::Paused);

        let resumed = executor
            .resume(instance.id)
            .await
            .expect("resume should succeed");
        let completed = wait_for_terminal_instance(&executor, resumed.id).await;
        assert_eq!(completed.status, WorkflowStatus::Completed);
    }

    async fn wait_for_terminal_instance(
        executor: &InMemoryWorkflowExecutor,
        instance_id: Ulid,
    ) -> WorkflowInstance {
        for _ in 0..50 {
            let instance = executor
                .get_instance(instance_id)
                .await
                .expect("instance query should succeed")
                .expect("instance should exist");
            if instance.status.is_terminal() {
                return instance;
            }
            sleep(TokioDuration::from_millis(10)).await;
        }

        executor
            .get_instance(instance_id)
            .await
            .expect("instance query should succeed")
            .expect("instance should exist")
    }
}
