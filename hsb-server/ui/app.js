const {
  createApp,
  ref,
  reactive,
  computed,
  onMounted,
  onBeforeUnmount,
  watch,
} = Vue;

const {
  createRouter,
  createWebHashHistory,
  useRoute,
  useRouter,
} = VueRouter;

const {
  ElMessage,
  ElMessageBox,
} = ElementPlus;

function normalizeRoutePrefix(value) {
  const trimmed = String(value || '').trim().replace(/^\/+|\/+$/g, '');
  return trimmed ? `/${trimmed}` : '';
}

const routePrefix = normalizeRoutePrefix(window.HSB_ROUTE_PREFIX || '');
const uiBasePath = `${routePrefix || ''}/ui/`;

function withRoutePrefix(path) {
  const normalizedPath = String(path || '/').startsWith('/') ? String(path || '/') : `/${path}`;
  if (!routePrefix) {
    return normalizedPath;
  }
  return normalizedPath === '/' ? `${routePrefix}/` : `${routePrefix}${normalizedPath}`;
}

const navGroups = [
  {
    index: '/dashboard',
    title: '总览',
    children: [{ index: '/dashboard', title: '运行态总览' }],
  },
  {
    index: '/integration',
    title: '集成配置',
    children: [
      { index: '/organizations', title: '机构管理' },
      { index: '/systems', title: '系统管理' },
      { index: '/topics', title: 'Topic 目录' },
      { index: '/protocols', title: '自定义协议' },
      { index: '/routes', title: '路由管理' },
      { index: '/endpoints', title: '端点管理' },
      { index: '/workflows', title: '工作流定义' },
    ],
  },
  {
    index: '/operations',
    title: '运行运维',
    children: [
      { index: '/messages', title: '消息中心' },
      { index: '/dlq', title: '死信队列' },
      { index: '/audit', title: '审计追踪' },
      { index: '/system', title: '系统配置' },
    ],
  },
];

const protocolOptions = [
  'HTTP',
  'WEBHOOK',
  'HL7V2',
  'FHIR_R4',
  'FHIR_R5',
  'DICOM',
  'SOAP',
  'GRPC',
  'TCP_RAW',
  'MESSAGE_QUEUE',
  'DATABASE',
  'OPENAI',
  'CUSTOM',
];

const systemTypeOptions = [
  'HIS',
  'LIS',
  'RIS',
  'PACS',
  'EMR',
  'HRP',
  'NIS',
  'ORS',
  'PHARMACY',
  'PROVINCIAL_PLATFORM',
  'MEDICAL_INSURANCE',
  'THIRD_PARTY',
  'OTHER',
];

const organizationTypeOptions = [
  { value: 'GOVERNMENT_DEPARTMENT', label: '政府部门' },
  { value: 'HOSPITAL', label: '医院' },
  { value: 'INDEPENDENT_LEGAL_ENTITY', label: '独立法人' },
  { value: 'OTHER', label: '其他' },
];

const endpointRoleOptions = [
  { value: 'PRODUCER', label: '生产者' },
  { value: 'CONSUMER', label: '消费者' },
  { value: 'HYBRID', label: '双角色' },
];

const endpointLifecycleOptions = [
  { value: 'DRAFT', label: '草稿' },
  { value: 'ACTIVE', label: '启用' },
  { value: 'DISABLED', label: '停用' },
  { value: 'DEPRECATED', label: '废弃中' },
  { value: 'RETIRED', label: '已退役' },
];

const endpointEncryptionOptions = [
  { value: 'NONE', label: '无' },
  { value: 'TLS1_2', label: 'TLS 1.2' },
  { value: 'TLS1_3', label: 'TLS 1.3' },
  { value: 'MUTUAL_TLS', label: '双向 TLS' },
  { value: 'SM3', label: 'SM3' },
];

const authTypeOptions = [
  { value: 'none', label: '无认证' },
  { value: 'basic', label: 'Basic' },
  { value: 'bearer', label: 'Bearer' },
  { value: 'api_key', label: 'API Key' },
];

const databaseTypeOptions = [
  { value: 'postgresql', label: 'PostgreSQL', port: 5432 },
  { value: 'oracle', label: 'Oracle', port: 1521 },
  { value: 'mysql', label: 'MySQL', port: 3306 },
  { value: 'sqlserver', label: 'SQL Server', port: 1433 },
  { value: 'hive', label: 'Hive', port: 10000 },
  { value: 'clickhouse', label: 'ClickHouse', port: 8123 },
];

const openAiEndpointOptions = [
  { value: 'chat_completions', label: 'Chat Completions', path: '/v1/chat/completions' },
  { value: 'responses', label: 'Responses', path: '/v1/responses' },
  { value: 'embeddings', label: 'Embeddings', path: '/v1/embeddings' },
];

const webhookMethodOptions = [
  { value: 'POST', label: 'POST' },
];

const routePriorityOptions = [1, 10, 50, 100, 500, 1000];

function optionLabel(options, value) {
  const item = options.find((entry) => entry.value === value);
  return item?.label || value || '-';
}

function parseListText(text) {
  return String(text || '')
    .split(/[,\n]/)
    .map((item) => item.trim())
    .filter(Boolean);
}

function stringifyList(items) {
  return Array.isArray(items) && items.length ? items.join('\n') : '';
}

function normalizePath(path) {
  if (!path || !String(path).trim()) {
    return null;
  }
  const trimmed = String(path).trim();
  return trimmed.startsWith('/') ? trimmed : `/${trimmed}`;
}

function defaultPortByProtocol(protocol, tlsEnabled = false) {
  const normalized = String(protocol || '').toUpperCase();
  if (normalized === 'HTTP') {
    return tlsEnabled ? 443 : 80;
  }
  if (normalized === 'WEBHOOK') {
    return tlsEnabled ? 443 : 80;
  }
  if (normalized === 'GRPC') {
    return tlsEnabled ? 9443 : 9090;
  }
  if (normalized === 'TCP_RAW' || normalized === 'HL7V2') {
    return 2575;
  }
  if (normalized === 'FHIR_R4' || normalized === 'FHIR_R5' || normalized === 'SOAP') {
    return tlsEnabled ? 443 : 80;
  }
  if (normalized === 'DICOM') {
    return 104;
  }
  if (normalized === 'DATABASE') {
    return 5432;
  }
  if (normalized === 'OPENAI') {
    return 443;
  }
  return tlsEnabled ? 443 : 80;
}

function defaultPortByDatabaseType(databaseType) {
  return databaseTypeOptions.find((item) => item.value === databaseType)?.port || 5432;
}

function defaultPathByOpenAiEndpoint(endpointType) {
  return openAiEndpointOptions.find((item) => item.value === endpointType)?.path || '/v1/chat/completions';
}

function buildConnectionAddress(connection, protocol) {
  if (!connection?.host) {
    return '-';
  }
  const normalized = String(protocol || '').toUpperCase();
  if (normalized === 'TCP_RAW' || normalized === 'HL7V2' || normalized === 'MESSAGE_QUEUE' || normalized === 'DATABASE') {
    return `${connection.host}:${connection.port}`;
  }
  const scheme = connection.tls_enabled ? 'https' : 'http';
  const path = connection.path || '';
  return `${scheme}://${connection.host}:${connection.port}${path}`;
}

function buildApiUrl(path, query) {
  const url = new URL(withRoutePrefix(`/api/v1${path}`), window.location.origin);
  Object.entries(query || {}).forEach(([key, value]) => {
    if (value !== undefined && value !== null && value !== '') {
      url.searchParams.set(key, value);
    }
  });
  return url.toString();
}

async function apiRequest(path, options = {}) {
  const headers = { Accept: 'application/json', ...(options.headers || {}) };
  const response = await fetch(buildApiUrl(path, options.query), {
    method: options.method || 'GET',
    headers,
    body: options.body,
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(text || `请求失败: ${response.status}`);
  }

  const contentType = response.headers.get('content-type') || '';
  if (contentType.includes('application/json')) {
    return response.json();
  }

  return response.text();
}

function createJsonBody(payload) {
  return JSON.stringify(payload);
}

function generateApiKey() {
  const cryptoApi = window.crypto || window.msCrypto;
  if (!cryptoApi?.getRandomValues) {
    throw new Error('当前浏览器不支持安全随机数生成');
  }

  const bytes = new Uint8Array(24);
  cryptoApi.getRandomValues(bytes);
  let binary = '';
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return window.btoa(binary).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

async function copyTextToClipboard(text) {
  const value = String(text || '').trim();
  if (!value) {
    throw new Error('没有可复制的 API Key');
  }

  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(value);
    return;
  }

  const textarea = document.createElement('textarea');
  textarea.value = value;
  textarea.setAttribute('readonly', 'readonly');
  textarea.style.position = 'fixed';
  textarea.style.left = '-9999px';
  document.body.appendChild(textarea);
  textarea.select();
  const copied = document.execCommand('copy');
  document.body.removeChild(textarea);
  if (!copied) {
    throw new Error('复制失败，请手动复制');
  }
}

function buildEndpointSecurity(form) {
  return {
    secret_ref: form.security_secret_ref || null,
    require_tls: !!form.security_require_tls,
    encryption_algorithm: form.security_encryption_algorithm || 'NONE',
    allow_insecure_skip_verify: !!form.security_allow_insecure_skip_verify,
    allowed_ip_ranges: parseListText(form.security_allowed_ip_ranges_text),
    mask_credentials_in_logs: !!form.security_mask_credentials_in_logs,
    credential_expires_at: form.security_credential_expires_at || null,
    credential_last_rotated_at: form.security_credential_last_rotated_at || null,
  };
}

function buildEndpointAuthConfig(form) {
  const authType = form.auth_type;
  if (authType === 'none') {
    return form.record_id ? { type: 'none' } : null;
  }

  if (authType === 'basic') {
    if (!form.auth_username) {
      throw new Error('Basic 认证需要用户名');
    }
    if (form.record_id && form.auth_preserve_existing && !form.auth_password) {
      return undefined;
    }
    if (!form.auth_password) {
      throw new Error('Basic 认证需要密码');
    }
    return {
      type: 'basic',
      username: form.auth_username || '',
      password: form.auth_password || '',
    };
  }

  if (authType === 'bearer') {
    if (form.record_id && form.auth_preserve_existing && !form.auth_token) {
      return undefined;
    }
    if (!form.auth_token) {
      throw new Error('Bearer 认证需要 Token');
    }
    return {
      type: 'bearer',
      token: form.auth_token || '',
    };
  }

  if (authType === 'api_key') {
    if (!form.auth_header_name) {
      throw new Error('API Key 认证需要 Header 名称');
    }
    if (form.record_id && form.auth_preserve_existing && !form.auth_api_secret) {
      return undefined;
    }
    if (!form.auth_api_secret) {
      throw new Error('API Key 认证需要密钥');
    }
    return {
      type: 'api_key',
      key: form.auth_api_secret || '',
      header_name: form.auth_header_name || 'X-API-Key',
    };
  }

  return null;
}

function endpointPayloadFromForm(form) {
  if (!form.name || !form.name.trim()) {
    throw new Error('端点名称不能为空');
  }
  if (!form.system_id) {
    throw new Error('请选择所属系统');
  }
  if (!form.host || !form.host.trim()) {
    throw new Error('主机地址不能为空');
  }

  const properties = parseJsonOrDefault(form.propertiesText, {});
  if (form.protocol === 'CUSTOM') {
    if (!form.custom_protocol_id) {
      throw new Error('CUSTOM 协议需要选择具体自定义协议');
    }
    properties.custom_protocol_id = form.custom_protocol_id;
  } else {
    delete properties.custom_protocol_id;
  }

  if (form.protocol === 'DATABASE') {
    if (!form.database_type) {
      throw new Error('DATABASE 端点需要选择数据库类型');
    }
    properties.database_type = form.database_type;
    properties.database_name = form.database_name || '';
    properties.schema = form.database_schema || '';
    properties.jdbc_url = form.jdbc_url || '';
  } else {
    delete properties.database_type;
    delete properties.database_name;
    delete properties.schema;
    delete properties.jdbc_url;
  }

  if (form.protocol === 'OPENAI') {
    if (!form.openai_model) {
      throw new Error('OPENAI 端点需要填写默认模型');
    }
    properties.endpoint_type = form.openai_endpoint_type || 'chat_completions';
    properties.model = form.openai_model;
    properties.organization = form.openai_organization || '';
    properties.project = form.openai_project || '';
  } else {
    delete properties.endpoint_type;
    delete properties.model;
    delete properties.organization;
    delete properties.project;
  }

  if (form.protocol === 'WEBHOOK') {
    properties.webhook_method = form.webhook_method || 'POST';
    properties.webhook_event = form.webhook_event || '';
    properties.signature_header = form.webhook_signature_header || 'X-HSB-Signature';
    properties.signing_secret_ref = form.webhook_signing_secret_ref || '';
  } else {
    delete properties.webhook_method;
    delete properties.webhook_event;
    delete properties.signature_header;
    delete properties.signing_secret_ref;
  }

  const payload = {
    name: form.name,
    description: form.description || null,
    system_id: form.system_id,
    protocol: form.protocol,
    system_type: form.system_type,
    roles: Array.isArray(form.roles) && form.roles.length ? form.roles : ['CONSUMER'],
    connection: {
      host: form.host,
      port: Number(form.port || defaultPortByProtocol(form.protocol, form.tls_enabled)),
      path: normalizePath(form.path),
      tls_enabled: !!form.tls_enabled,
      tls_cert_path: form.tls_cert_path || null,
      connect_timeout_secs: Number(form.connect_timeout_secs || 10),
      read_timeout_secs: Number(form.read_timeout_secs || 30),
      write_timeout_secs: Number(form.write_timeout_secs || 30),
      pool_size: Number(form.pool_size || 10),
      reconnect_interval_secs: Number(form.reconnect_interval_secs || 5),
      keepalive_secs: form.keepalive_secs === '' || form.keepalive_secs === null
        ? null
        : Number(form.keepalive_secs),
    },
    config: {
      max_retries: Number(form.max_retries || 3),
      retry_interval_ms: Number(form.retry_interval_ms || 1000),
      compression_enabled: !!form.compression_enabled,
      max_message_size: Number(form.max_message_size || 10485760),
      concurrency_limit: Number(form.concurrency_limit || 100),
      rate_limit: form.rate_limit === '' || form.rate_limit === null ? null : Number(form.rate_limit),
      circuit_breaker_threshold: form.circuit_breaker_threshold === '' || form.circuit_breaker_threshold === null
        ? null
        : Number(form.circuit_breaker_threshold),
      log_body: !!form.log_body,
    },
    enabled: !!form.enabled,
    lifecycle_status: form.lifecycle_status,
    security: buildEndpointSecurity(form),
    properties,
    change_note: form.change_note || null,
  };

  if (!form.record_id && form.requested_id && form.requested_id.trim()) {
    payload.id = form.requested_id.trim();
  }

  const auth = buildEndpointAuthConfig(form);
  if (auth !== undefined) {
    payload.auth = auth;
  }

  if (form.record_id) {
    payload.updated_by = form.actor || null;
  } else {
    payload.created_by = form.actor || null;
  }

  return payload;
}

function routePayloadFromForm(form) {
  const metadata = parseJsonOrDefault(form.metadataText, {});
  const conditions = [];

  if (form.source_pattern) {
    conditions.push({
      field: 'source',
      operator: 'matches',
      value: form.source_pattern,
    });
  }

  if (form.message_type) {
    conditions.push({
      field: 'message_type',
      operator: 'eq',
      value: form.message_type,
    });
  }

  if (form.protocol) {
    conditions.push({
      field: 'protocol',
      operator: 'eq',
      value: form.protocol,
    });
  }

  const routeTargets = form.target_ids
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean)
    .map((endpoint_id) => ({ endpoint_id, weight: 100 }));

  return {
    name: form.name,
    description: form.description || null,
    enabled: form.enabled,
    priority: Number(form.priority || 100),
    source: {
      system_type: form.system_type || null,
      endpoint_id: form.source_endpoint_id || null,
      pattern: form.source_pattern || null,
    },
    conditions,
    targets: routeTargets,
    metadata,
  };
}

function buildAuthConfig(form) {
  return buildEndpointAuthConfig(form);
}

function organizationFormFromItem(item) {
  return {
    record_id: item.id || '',
    requested_id: item.id || '',
    name: item.name || '',
    description: item.description || '',
    organization_type: item.organization_type || 'HOSPITAL',
    parent_organization_id: item.parent_organization_id || '',
    enabled: item.enabled !== false,
    propertiesText: stringifyPretty(item.properties || {}),
  };
}

function organizationPayloadFromForm(form) {
  const payload = {
    name: form.name,
    description: form.description || null,
    organization_type: form.organization_type,
    parent_organization_id: form.parent_organization_id || null,
    enabled: !!form.enabled,
    properties: parseJsonOrDefault(form.propertiesText, {}),
  };
  if (!form.record_id && form.requested_id && form.requested_id.trim()) {
    payload.id = form.requested_id.trim();
  }
  return payload;
}

function systemFormFromItem(item) {
  return {
    record_id: item.id || '',
    requested_id: item.id || '',
    organization_id: item.organization_id || '',
    name: item.name || '',
    description: item.description || '',
    system_type: item.system_type || 'OTHER',
    topic_namespace: item.topic_namespace || '',
    topic_prefix: item.topic_prefix || '',
    enabled: item.enabled !== false,
    propertiesText: stringifyPretty(item.properties || {}),
  };
}

function systemPayloadFromForm(form) {
  const payload = {
    organization_id: form.organization_id,
    name: form.name,
    description: form.description || null,
    system_type: form.system_type,
    topic_namespace: form.topic_namespace || null,
    topic_prefix: form.topic_prefix || null,
    enabled: !!form.enabled,
    properties: parseJsonOrDefault(form.propertiesText, {}),
  };
  if (!form.record_id && form.requested_id && form.requested_id.trim()) {
    payload.id = form.requested_id.trim();
  }
  return payload;
}

function endpointFormFromItem(item = {}) {
  const connection = item.connection || {};
  const config = item.config || {};
  const security = item.security || {};
  const auth = item.auth || {};
  return {
    record_id: item.id || '',
    requested_id: item.id || '',
    organization_id: item.organization_id || '',
    system_id: item.system_id || '',
    name: item.name || '',
    description: item.description || '',
    protocol: item.protocol || 'HTTP',
    custom_protocol_id: item.properties?.custom_protocol_id || '',
    database_type: item.properties?.database_type || 'postgresql',
    database_name: item.properties?.database_name || '',
    database_schema: item.properties?.schema || '',
    jdbc_url: item.properties?.jdbc_url || '',
    openai_endpoint_type: item.properties?.endpoint_type || 'chat_completions',
    openai_model: item.properties?.model || 'gpt-4o-mini',
    openai_organization: item.properties?.organization || '',
    openai_project: item.properties?.project || '',
    webhook_method: item.properties?.webhook_method || 'POST',
    webhook_event: item.properties?.webhook_event || '',
    webhook_signature_header: item.properties?.signature_header || 'X-HSB-Signature',
    webhook_signing_secret_ref: item.properties?.signing_secret_ref || '',
    system_type: item.system_type || 'OTHER',
    roles: Array.isArray(item.roles) && item.roles.length ? item.roles : ['CONSUMER'],
    host: connection.host || '',
    port: connection.port || defaultPortByProtocol(item.protocol || 'HTTP', !!connection.tls_enabled),
    path: connection.path || '',
    enabled: item.enabled !== false,
    lifecycle_status: item.lifecycle_status || 'ACTIVE',
    tls_enabled: !!connection.tls_enabled,
    tls_cert_path: connection.tls_cert_path || '',
    connect_timeout_secs: connection.connect_timeout_secs || 10,
    read_timeout_secs: connection.read_timeout_secs || 30,
    write_timeout_secs: connection.write_timeout_secs || 30,
    pool_size: connection.pool_size || 10,
    reconnect_interval_secs: connection.reconnect_interval_secs || 5,
    keepalive_secs: connection.keepalive_secs ?? 60,
    max_retries: config.max_retries || 3,
    retry_interval_ms: config.retry_interval_ms || 1000,
    compression_enabled: !!config.compression_enabled,
    max_message_size: config.max_message_size || 10485760,
    concurrency_limit: config.concurrency_limit || 100,
    rate_limit: config.rate_limit ?? null,
    circuit_breaker_threshold: config.circuit_breaker_threshold ?? 5,
    log_body: !!config.log_body,
    security_secret_ref: security.secret_ref || '',
    security_require_tls: !!security.require_tls,
    security_encryption_algorithm: security.encryption_algorithm || 'NONE',
    security_allow_insecure_skip_verify: !!security.allow_insecure_skip_verify,
    security_allowed_ip_ranges_text: stringifyList(security.allowed_ip_ranges || []),
    security_mask_credentials_in_logs: security.mask_credentials_in_logs !== false,
    security_credential_expires_at: security.credential_expires_at || '',
    security_credential_last_rotated_at: security.credential_last_rotated_at || null,
    auth_type: auth.auth_type || 'none',
    original_auth_type: auth.auth_type || 'none',
    auth_preserve_existing: !!auth.auth_type,
    auth_username: auth.principal || '',
    auth_password: '',
    auth_token: '',
    auth_header_name: auth.header_name || 'X-API-Key',
    auth_api_secret: '',
    propertiesText: stringifyPretty(item.properties || {}),
    actor: '',
    change_note: '',
  };
}

function routeFormFromItem(item) {
  const conditions = Array.isArray(item.conditions) ? item.conditions : [];
  const sourceCondition = conditions.find((entry) => entry.field === 'source');
  const protocolCondition = conditions.find((entry) => entry.field === 'protocol');
  const messageTypeCondition = conditions.find((entry) => entry.field === 'message_type');
  return {
    id: item.id || '',
    name: item.name || '',
    description: item.description || '',
    enabled: item.enabled !== false,
    priority: item.priority || 100,
    system_type: item.source?.system_type || '',
    source_endpoint_id: item.source?.endpoint_id || '',
    source_pattern: item.source?.pattern || sourceCondition?.value || '',
    protocol: protocolCondition?.value || '',
    message_type: messageTypeCondition?.value || '',
    target_ids: Array.isArray(item.targets)
      ? item.targets.map((target) => target.endpoint_id).join(', ')
      : '',
    metadataText: stringifyPretty(item.metadata || {}),
  };
}

function parseJsonOrDefault(text, fallback) {
  if (!text || !text.trim()) {
    return fallback;
  }

  return JSON.parse(text);
}

function stringifyPretty(value) {
  return JSON.stringify(value || {}, null, 2);
}

function defaultWorkflowStepPayload() {
  return {
    id: 'step_1',
    name: '发送到 HIS',
    step_type: {
      type: 'send',
      endpoint_id: 'HIS_ENDPOINT',
      transformer_ids: [],
    },
    config: {
      async_execution: false,
      skippable: false,
      input_mapping: {},
      output_mapping: {},
      properties: {},
    },
    retry: {
      max_attempts: 3,
      initial_delay_ms: 1000,
      max_delay_ms: 30000,
      multiplier: 2,
      retryable_errors: [],
    },
    timeout_ms: 30000,
    condition: null,
    compensation_step: null,
    next_steps: [],
  };
}

function defaultWorkflowForm() {
  return {
    id: '',
    name: '',
    description: '',
    version: 1,
    enabled: true,
    timeout_ms: 3600000,
    persist_state: true,
    pausable: true,
    max_concurrent_instances: null,
    instance_timeout_secs: 3600,
    compensationText: '',
    stepsText: stringifyPretty([defaultWorkflowStepPayload()]),
  };
}

function workflowFormFromItem(item) {
  const options = item.options || {};
  return {
    id: item.id || '',
    name: item.name || '',
    description: item.description || '',
    version: item.version || 1,
    enabled: item.enabled !== false,
    timeout_ms: item.timeout_ms || 3600000,
    persist_state: options.persist_state !== false,
    pausable: options.pausable !== false,
    max_concurrent_instances: options.max_concurrent_instances ?? null,
    instance_timeout_secs: options.instance_timeout_secs || 3600,
    compensationText: item.compensation ? stringifyPretty(item.compensation) : '',
    stepsText: stringifyPretty(item.steps || [defaultWorkflowStepPayload()]),
  };
}

function workflowPayloadFromForm(form, isUpdate = false) {
  const steps = parseJsonOrDefault(form.stepsText, []);
  if (!Array.isArray(steps)) {
    throw new Error('工作流步骤必须是 JSON 数组');
  }

  const payload = {
    name: form.name,
    description: form.description || null,
    version: Number(form.version || 1),
    enabled: !!form.enabled,
    timeout_ms: Number(form.timeout_ms || 3600000),
    options: {
      persist_state: !!form.persist_state,
      pausable: !!form.pausable,
      max_concurrent_instances: form.max_concurrent_instances === null || form.max_concurrent_instances === ''
        ? null
        : Number(form.max_concurrent_instances),
      instance_timeout_secs: Number(form.instance_timeout_secs || 3600),
    },
    steps,
  };

  if (form.compensationText && form.compensationText.trim()) {
    payload.compensation = parseJsonOrDefault(form.compensationText, null);
  } else if (isUpdate) {
    payload.clear_compensation = true;
  }

  if (!isUpdate && form.id) {
    payload.id = form.id;
  }

  return payload;
}

function workflowStepType(step) {
  return step?.step_type?.type || 'unknown';
}

function defaultWorkflowStartForm(workflowId = '') {
  return {
    workflow_id: workflowId,
    source_system: 'HIS',
    target_system: '',
    protocol: 'HTTP',
    custom_protocol_id: '',
    message_type: '',
    correlation_id: '',
    payloadText: stringifyPretty({ patient_id: 'P10001' }),
    raw_payload_text: '',
  };
}

function workflowStartPayloadFromForm(form) {
  if (form.protocol === 'CUSTOM' && !form.custom_protocol_id) {
    throw new Error('CUSTOM 协议需要选择具体自定义协议');
  }
  const payload = form.payloadText && form.payloadText.trim()
    ? parseJsonOrDefault(form.payloadText, null)
    : null;
  if (form.protocol === 'CUSTOM' && payload && typeof payload === 'object' && !Array.isArray(payload)) {
    payload.custom_protocol_id = form.custom_protocol_id;
  }
  return {
    source_system: form.source_system,
    target_system: form.target_system || null,
    protocol: form.protocol,
    message_type: form.message_type || null,
    correlation_id: form.correlation_id || null,
    payload,
    raw_payload_text: form.raw_payload_text || null,
  };
}

function customProtocolFormFromItem(item = {}) {
  return {
    id: item.id || '',
    requested_id: item.id || '',
    name: item.name || '',
    description: item.description || '',
    transport_hint: item.transport_hint || '',
    content_type: item.content_type || '',
    enabled: item.enabled !== false,
    fieldsText: stringifyPretty(item.fields || [
      { name: 'message_type', label: '消息类型', data_type: 'string', required: true, description: '' },
    ]),
    samplePayloadText: stringifyPretty(item.sample_payload || {}),
  };
}

function customProtocolPayloadFromForm(form, isUpdate = false) {
  const fields = parseJsonOrDefault(form.fieldsText, []);
  if (!Array.isArray(fields)) {
    throw new Error('字段定义必须是 JSON 数组');
  }
  const payload = {
    name: form.name,
    description: form.description || null,
    transport_hint: form.transport_hint || null,
    content_type: form.content_type || null,
    fields,
    sample_payload: form.samplePayloadText && form.samplePayloadText.trim()
      ? parseJsonOrDefault(form.samplePayloadText, null)
      : null,
    enabled: !!form.enabled,
  };
  if (!isUpdate && form.requested_id && form.requested_id.trim()) {
    payload.id = form.requested_id.trim();
  }
  return payload;
}

function topicFormFromItem(item = {}) {
  return {
    id: item.id || '',
    topic: item.topic || '',
    description: item.description || '',
    owner_system_id: item.owner_system_id || '',
    enabled: item.enabled !== false,
    propertiesText: stringifyPretty(item.properties || {}),
  };
}

function topicPayloadFromForm(form) {
  return {
    topic: form.topic,
    description: form.description || null,
    owner_system_id: form.owner_system_id || null,
    enabled: !!form.enabled,
    properties: parseJsonOrDefault(form.propertiesText, {}),
  };
}

function canPauseInstance(status) {
  return ['RUNNING', 'PENDING', 'WAITING'].includes(String(status || '').toUpperCase());
}

function canResumeInstance(status) {
  return ['PAUSED', 'FAILED'].includes(String(status || '').toUpperCase());
}

function canCancelInstance(status) {
  return !['COMPLETED', 'CANCELLED', 'COMPENSATED'].includes(String(status || '').toUpperCase());
}

function canCompensateInstance(status) {
  return ['FAILED'].includes(String(status || '').toUpperCase());
}

function formatTime(value) {
  if (!value) {
    return '-';
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return `${date.toLocaleDateString('zh-CN')} ${date.toLocaleTimeString('zh-CN')}`;
}

function humanizeState(value) {
  if (!value) {
    return '未知';
  }
  return String(value)
    .replace(/_/g, ' ')
    .replace(/\b\w/g, (char) => char.toUpperCase());
}

function tagTypeByState(value) {
  const normalized = String(value || '').toLowerCase();
  if (normalized.includes('active') || normalized.includes('healthy') || normalized.includes('success')) {
    return 'success';
  }
  if (normalized.includes('pending') || normalized.includes('processing') || normalized.includes('warning')) {
    return 'warning';
  }
  if (normalized.includes('disabled') || normalized.includes('closed')) {
    return 'info';
  }
  if (normalized.includes('failed') || normalized.includes('dead') || normalized.includes('open')) {
    return 'danger';
  }
  return '';
}

function useAsyncResource(loader, initialValue) {
  const state = reactive({
    value: initialValue,
    loading: false,
    error: '',
  });

  async function refresh() {
    state.loading = true;
    state.error = '';
    try {
      state.value = await loader();
    } catch (error) {
      state.error = error.message || String(error);
      ElMessage.error(state.error);
    } finally {
      state.loading = false;
    }
  }

  return { state, refresh };
}

function normalizeSearch(value) {
  return String(value || '').trim().toLowerCase();
}

function matchesKeyword(item, keyword, fields) {
  const normalized = normalizeSearch(keyword);
  if (!normalized) {
    return true;
  }
  return fields
    .flatMap((field) => {
      const value = typeof field === 'function' ? field(item) : item?.[field];
      return Array.isArray(value) ? value : [value];
    })
    .filter((value) => value !== undefined && value !== null && value !== '')
    .join(' ')
    .toLowerCase()
    .includes(normalized);
}

function uniqueValues(items, field) {
  return Array.from(new Set((items || []).map((item) => item?.[field]).filter(Boolean))).sort();
}

function usePagedList(items, options = {}) {
  const currentPage = ref(1);
  const pageSize = ref(options.defaultPageSize || 10);
  const rowHeight = options.rowHeight || 46;
  const minRows = options.minRows || 6;
  const maxRows = options.maxRows || 18;
  const minTableHeight = options.minTableHeight || 360;
  const reservedHeight = options.reservedHeight || 410;
  const compactReservedHeight = options.compactReservedHeight || 360;

  const total = computed(() => (items.value || []).length);
  const pageCount = computed(() => Math.max(1, Math.ceil(total.value / pageSize.value)));
  const pagedItems = computed(() => {
    const start = (currentPage.value - 1) * pageSize.value;
    return (items.value || []).slice(start, start + pageSize.value);
  });
  const tableHeight = computed(() => Math.max(minTableHeight, pageSize.value * rowHeight + 48));

  function recalculatePageSize() {
    const viewportHeight = window.innerHeight || 760;
    const reserved = viewportHeight < 720 ? compactReservedHeight : reservedHeight;
    const nextPageSize = Math.max(minRows, Math.min(maxRows, Math.floor((viewportHeight - reserved) / rowHeight)));
    pageSize.value = nextPageSize;
    currentPage.value = Math.min(currentPage.value, pageCount.value);
  }

  function goFirstPage() {
    currentPage.value = 1;
  }

  function goLastPage() {
    currentPage.value = pageCount.value;
  }

  onMounted(() => {
    recalculatePageSize();
    window.addEventListener('resize', recalculatePageSize);
  });

  onBeforeUnmount(() => {
    window.removeEventListener('resize', recalculatePageSize);
  });

  watch([total, pageSize], () => {
    currentPage.value = Math.min(currentPage.value, pageCount.value);
  });

  return { currentPage, pageSize, total, pageCount, pagedItems, tableHeight, goFirstPage, goLastPage };
}

const DashboardPage = {
  name: 'DashboardPage',
  setup() {
    const metrics = reactive({
      organizations: 0,
      systems: 0,
      routes: 0,
      endpoints: 0,
      pending: 0,
      dlq: 0,
    });
    const status = ref({});
    const messages = ref([]);
    const circuitBreakers = ref([]);
    const loading = ref(false);

    async function load() {
      loading.value = true;
      try {
        const [organizationData, systemData, routeData, endpointData, messageData, dlqData, statusData, cbData] = await Promise.all([
          apiRequest('/organizations'),
          apiRequest('/systems'),
          apiRequest('/routes'),
          apiRequest('/endpoints'),
          apiRequest('/messages', { query: { page: 1, page_size: 8 } }),
          apiRequest('/dlq'),
          apiRequest('/status'),
          apiRequest('/circuit-breakers'),
        ]);

        metrics.organizations = Array.isArray(organizationData.items) ? organizationData.items.length : organizationData.total || 0;
        metrics.systems = Array.isArray(systemData.items) ? systemData.items.length : systemData.total || 0;
        metrics.routes = Array.isArray(routeData.items) ? routeData.items.length : routeData.total || 0;
        metrics.endpoints = Array.isArray(endpointData.items) ? endpointData.items.length : endpointData.total || 0;
        metrics.pending = Array.isArray(messageData.items)
          ? messageData.items.filter((item) => String(item.status || '').toLowerCase().includes('pending')).length
          : 0;
        metrics.dlq = Array.isArray(dlqData.items) ? dlqData.items.length : dlqData.total || 0;
        status.value = statusData || {};
        messages.value = Array.isArray(messageData.items) ? messageData.items : [];
        circuitBreakers.value = Array.isArray(cbData.items) ? cbData.items : Array.isArray(cbData) ? cbData : [];
      } catch (error) {
        ElMessage.error(error.message || '仪表盘数据加载失败');
      } finally {
        loading.value = false;
      }
    }

    onMounted(load);

    return {
      loading,
      metrics,
      status,
      messages,
      circuitBreakers,
      formatTime,
      tagTypeByState,
      humanizeState,
      load,
    };
  },
  template: `
    <div class="content-grid" v-loading="loading">
      <div class="page-card">
        <h2 class="page-section-title">统一出口运行总览</h2>
        <p class="page-section-subtitle">axum 主入口已统一承载管理台与 API。这里展示当前路由规模、端点规模、消息堆积和死信状态，方便值守时快速判断系统是否可继续承压。</p>
        <div class="hero-grid">
          <div class="metric-card">
            <div>
              <div class="metric-card__label">已登记机构</div>
              <div class="metric-card__value">{{ metrics.organizations }}</div>
            </div>
            <div class="metric-card__footer">治理边界与法人主体目录</div>
          </div>
          <div class="metric-card">
            <div>
              <div class="metric-card__label">已登记系统</div>
              <div class="metric-card__value">{{ metrics.systems }}</div>
            </div>
            <div class="metric-card__footer">每个系统从属于单一机构</div>
          </div>
          <div class="metric-card">
            <div>
              <div class="metric-card__label">已注册路由</div>
              <div class="metric-card__value">{{ metrics.routes }}</div>
            </div>
            <div class="metric-card__footer">控制面当前有效配置规模</div>
          </div>
          <div class="metric-card">
            <div>
              <div class="metric-card__label">已维护端点</div>
              <div class="metric-card__value">{{ metrics.endpoints }}</div>
            </div>
            <div class="metric-card__footer">包含 HTTP、MQ、HL7、FHIR 等协议出口</div>
          </div>
          <div class="metric-card">
            <div>
              <div class="metric-card__label">待恢复消息</div>
              <div class="metric-card__value">{{ metrics.pending }}</div>
            </div>
            <div class="metric-card__footer">用于观察重试和恢复压力</div>
          </div>
          <div class="metric-card">
            <div>
              <div class="metric-card__label">死信积压</div>
              <div class="metric-card__value">{{ metrics.dlq }}</div>
            </div>
            <div class="metric-card__footer">超过阈值应尽快重放或排障</div>
          </div>
        </div>
      </div>

      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">系统状态</h2>
            <p class="page-section-subtitle">当前实例状态、熔断器状态和最近消息动态。</p>
          </div>
          <el-button type="primary" @click="load">刷新</el-button>
        </div>
        <div class="system-grid">
          <div class="panel-card" style="padding: 18px 20px;">
            <h3 class="page-section-title" style="font-size: 16px;">实例状态快照</h3>
            <el-descriptions :column="1" border>
              <el-descriptions-item label="服务状态">{{ status.status || '未知' }}</el-descriptions-item>
              <el-descriptions-item label="版本">{{ status.version || '-' }}</el-descriptions-item>
              <el-descriptions-item label="启动时间">{{ formatTime(status.started_at) }}</el-descriptions-item>
              <el-descriptions-item label="运行秒数">{{ status.uptime_seconds ?? '-' }}</el-descriptions-item>
            </el-descriptions>
          </div>
          <div class="panel-card" style="padding: 18px 20px;">
            <h3 class="page-section-title" style="font-size: 16px;">熔断器状态</h3>
            <div v-if="!circuitBreakers.length" class="page-section-subtitle">暂无熔断器状态数据。</div>
            <div v-else class="stats-grid">
              <div v-for="item in circuitBreakers" :key="item.name || item.id" class="mini-stat">
                <div class="mini-stat__label">{{ item.name || item.id || '未命名熔断器' }}</div>
                <div class="mini-stat__value">
                  <el-tag :type="tagTypeByState(item.state)">{{ humanizeState(item.state) }}</el-tag>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <div class="page-card">
        <h2 class="page-section-title">最近消息</h2>
        <el-table :data="messages" stripe>
          <el-table-column prop="id" label="消息ID" min-width="220" />
          <el-table-column prop="protocol" label="协议" width="120" />
          <el-table-column prop="message_type" label="类型" width="140" />
          <el-table-column label="状态" width="140">
            <template #default="scope">
              <el-tag :type="tagTypeByState(scope.row.status)">{{ humanizeState(scope.row.status) }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="接收时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.received_at || scope.row.created_at) }}</template>
          </el-table-column>
        </el-table>
      </div>
    </div>
  `,
};

const OrganizationsPage = {
  name: 'OrganizationsPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/organizations'), { items: [], total: 0 });
    const drawerVisible = ref(false);
    const submitting = ref(false);
    const filters = reactive({ keyword: '', organization_type: '', enabled: '' });
    const form = reactive(organizationFormFromItem({}));

    const filteredItems = computed(() => {
      return (resource.state.value.items || []).filter((item) => {
        if (filters.organization_type && item.organization_type !== filters.organization_type) {
          return false;
        }
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        return matchesKeyword(item, filters.keyword, ['id', 'name', 'description', 'organization_type']);
      });
    });
    const pager = usePagedList(filteredItems);

    function resetForm(item = {}) {
      Object.assign(form, organizationFormFromItem(item));
    }

    function openCreate() {
      resetForm({});
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    async function save() {
      try {
        if (!form.name || !form.name.trim()) {
          throw new Error('机构名称不能为空');
        }
        submitting.value = true;
        const payload = organizationPayloadFromForm(form);
        if (form.record_id) {
          await apiRequest(`/organizations/${form.record_id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('机构已更新');
        } else {
          await apiRequest('/organizations', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('机构已创建');
        }
        drawerVisible.value = false;
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '保存机构失败');
      } finally {
        submitting.value = false;
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除机构 ${item.name || item.id} 吗？`, '删除机构', { type: 'warning' });
        await apiRequest(`/organizations/${item.id}`, { method: 'DELETE' });
        ElMessage.success('机构已删除');
        await resource.refresh();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除机构失败');
        }
      }
    }

    onMounted(resource.refresh);

    const parentNameMap = computed(() => {
      const map = new Map();
      (resource.state.value.items || []).forEach((item) => map.set(item.id, item.name || item.id));
      return map;
    });

    const parentOptions = computed(() => {
      return (resource.state.value.items || []).filter((item) => item.id !== form.record_id);
    });

    function resetFilters() {
      Object.assign(filters, { keyword: '', organization_type: '', enabled: '' });
    }

    return {
      ...resource,
      ...pager,
      drawerVisible,
      submitting,
      filters,
      form,
      filteredItems,
      parentNameMap,
      parentOptions,
      organizationTypeOptions,
      optionLabel,
      formatTime,
      openCreate,
      openEdit,
      save,
      remove,
      resetFilters,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">机构管理</h2>
            <p class="page-section-subtitle">定义政府部门、医院、独立法人等治理主体。系统必须挂在机构下，端点再挂在系统下。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建机构</el-button>
          </div>
        </div>

        <el-alert v-if="state.error" :title="state.error" type="error" show-icon :closable="false" style="margin-bottom: 16px;" />

        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入机构名称、ID 或描述" clearable />
            </el-form-item>
            <el-form-item label="机构类型">
              <el-select v-model="filters.organization_type" clearable placeholder="全部类型">
                <el-option v-for="item in organizationTypeOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-select v-model="filters.enabled" clearable placeholder="全部状态">
                <el-option label="启用" value="true" />
                <el-option label="停用" value="false" />
              </el-select>
            </el-form-item>
          </div>
        </div>

        <el-table :data="pagedItems" stripe v-loading="state.loading" row-key="id" class="list-table" :height="tableHeight">
          <el-table-column prop="name" label="机构名称" min-width="180" />
          <el-table-column prop="id" label="机构ID" min-width="180" />
          <el-table-column label="机构类型" width="160">
            <template #default="scope">{{ optionLabel(organizationTypeOptions, scope.row.organization_type) }}</template>
          </el-table-column>
          <el-table-column label="上级机构" min-width="160">
            <template #default="scope">{{ parentNameMap.get(scope.row.parent_organization_id) || scope.row.parent_organization_id || '-' }}</template>
          </el-table-column>
          <el-table-column label="状态" width="120">
            <template #default="scope">
              <el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="更新时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.updated_at) }}</template>
          </el-table-column>
          <el-table-column label="操作" width="200" fixed="right">
            <template #default="scope">
              <el-space wrap>
                <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
                <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="currentPage" background layout="pager" :page-size="pageSize" :pager-count="7" :total="total" />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.record_id ? '编辑机构' : '新建机构'" size="560px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item v-if="!form.record_id" label="机构ID">
              <el-input v-model="form.requested_id" placeholder="可选，留空则后端自动生成" />
            </el-form-item>
            <el-form-item label="机构名称"><el-input v-model="form.name" /></el-form-item>
            <el-form-item label="机构类型">
              <el-select v-model="form.organization_type">
                <el-option v-for="item in organizationTypeOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item label="状态">
              <el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" />
            </el-form-item>
          </div>
          <el-form-item label="上级机构">
            <el-select v-model="form.parent_organization_id" clearable filterable placeholder="可选">
              <el-option v-for="item in parentOptions" :key="item.id" :label="item.name" :value="item.id" />
            </el-select>
          </el-form-item>
          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="3" /></el-form-item>
          <el-form-item label="属性 JSON" class="code-block"><el-input v-model="form.propertiesText" type="textarea" /></el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>
    </div>
  `,
};

const SystemsPage = {
  name: 'SystemsPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/systems'), { items: [], total: 0 });
    const organizationResource = useAsyncResource(async () => apiRequest('/organizations'), { items: [], total: 0 });
    const drawerVisible = ref(false);
    const submitting = ref(false);
    const filters = reactive({ keyword: '', organization_id: '', system_type: '', enabled: '' });
    const form = reactive(systemFormFromItem({}));

    function resetForm(item = {}) {
      Object.assign(form, systemFormFromItem(item));
    }

    function openCreate() {
      resetForm({});
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    async function save() {
      try {
        if (!form.organization_id) {
          throw new Error('请选择所属机构');
        }
        if (!form.name || !form.name.trim()) {
          throw new Error('系统名称不能为空');
        }
        submitting.value = true;
        const payload = systemPayloadFromForm(form);
        if (form.record_id) {
          await apiRequest(`/systems/${form.record_id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('系统已更新');
        } else {
          await apiRequest('/systems', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('系统已创建');
        }
        drawerVisible.value = false;
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '保存系统失败');
      } finally {
        submitting.value = false;
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除系统 ${item.name || item.id} 吗？`, '删除系统', { type: 'warning' });
        await apiRequest(`/systems/${item.id}`, { method: 'DELETE' });
        ElMessage.success('系统已删除');
        await resource.refresh();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除系统失败');
        }
      }
    }

    onMounted(async () => {
      await Promise.all([resource.refresh(), organizationResource.refresh()]);
    });

    const organizationNameMap = computed(() => {
      const map = new Map();
      (organizationResource.state.value.items || []).forEach((item) => map.set(item.id, item.name || item.id));
      return map;
    });

    const filteredItems = computed(() => {
      return (resource.state.value.items || []).filter((item) => {
        if (filters.organization_id && item.organization_id !== filters.organization_id) {
          return false;
        }
        if (filters.system_type && item.system_type !== filters.system_type) {
          return false;
        }
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        return matchesKeyword(item, filters.keyword, [
          'id',
          'name',
          'description',
          'system_type',
          'topic_namespace',
          'topic_prefix',
          (value) => organizationNameMap.value.get(value.organization_id),
        ]);
      });
    });
    const pager = usePagedList(filteredItems);

    function resetFilters() {
      Object.assign(filters, { keyword: '', organization_id: '', system_type: '', enabled: '' });
    }

    return {
      ...resource,
      ...pager,
      organizationResource,
      drawerVisible,
      submitting,
      filters,
      form,
      filteredItems,
      organizationNameMap,
      systemTypeOptions,
      formatTime,
      openCreate,
      openEdit,
      save,
      remove,
      resetFilters,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">系统管理</h2>
            <p class="page-section-subtitle">定义属于机构的集成系统，并约定 topic 命名空间、前缀和系统类型。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建系统</el-button>
          </div>
        </div>

        <el-alert v-if="state.error" :title="state.error" type="error" show-icon :closable="false" style="margin-bottom: 16px;" />

        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入系统名称、ID、机构或 Topic 约定" clearable />
            </el-form-item>
            <el-form-item label="所属机构">
              <el-select v-model="filters.organization_id" filterable clearable placeholder="全部机构">
                <el-option v-for="item in organizationResource.state.value.items || []" :key="item.id" :label="item.name" :value="item.id" />
              </el-select>
            </el-form-item>
            <el-form-item label="系统类型">
              <el-select v-model="filters.system_type" clearable placeholder="全部类型">
                <el-option v-for="item in systemTypeOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-select v-model="filters.enabled" clearable placeholder="全部状态">
                <el-option label="启用" value="true" />
                <el-option label="停用" value="false" />
              </el-select>
            </el-form-item>
          </div>
        </div>

        <el-table :data="pagedItems" stripe v-loading="state.loading || organizationResource.state.loading" row-key="id" class="list-table" :height="tableHeight">
          <el-table-column prop="name" label="系统名称" min-width="180" />
          <el-table-column prop="id" label="系统ID" min-width="180" />
          <el-table-column label="所属机构" min-width="180">
            <template #default="scope">{{ organizationNameMap.get(scope.row.organization_id) || scope.row.organization_id || '-' }}</template>
          </el-table-column>
          <el-table-column prop="system_type" label="系统类型" width="150" />
          <el-table-column label="Topic 约定" min-width="220">
            <template #default="scope">{{ scope.row.topic_namespace || '-' }} / {{ scope.row.topic_prefix || '-' }}</template>
          </el-table-column>
          <el-table-column label="状态" width="120">
            <template #default="scope">
              <el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="更新时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.updated_at) }}</template>
          </el-table-column>
          <el-table-column label="操作" width="200" fixed="right">
            <template #default="scope">
              <el-space wrap>
                <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
                <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="currentPage" background layout="pager" :page-size="pageSize" :pager-count="7" :total="total" />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.record_id ? '编辑系统' : '新建系统'" size="620px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item v-if="!form.record_id" label="系统ID">
              <el-input v-model="form.requested_id" placeholder="可选，留空则后端自动生成" />
            </el-form-item>
            <el-form-item label="所属机构">
              <el-select v-model="form.organization_id" filterable placeholder="请选择机构">
                <el-option
                  v-for="item in organizationResource.state.value.items || []"
                  :key="item.id"
                  :label="item.name"
                  :value="item.id"
                />
              </el-select>
            </el-form-item>
            <el-form-item label="系统名称"><el-input v-model="form.name" /></el-form-item>
            <el-form-item label="系统类型">
              <el-select v-model="form.system_type">
                <el-option v-for="item in systemTypeOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="Topic Namespace"><el-input v-model="form.topic_namespace" placeholder="hospital.his" /></el-form-item>
            <el-form-item label="Topic Prefix"><el-input v-model="form.topic_prefix" placeholder="order" /></el-form-item>
            <el-form-item label="状态">
              <el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" />
            </el-form-item>
          </div>

          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="3" /></el-form-item>
          <el-form-item label="属性 JSON" class="code-block"><el-input v-model="form.propertiesText" type="textarea" /></el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>
    </div>
  `,
};

const EndpointsPage = {
  name: 'EndpointsPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/endpoints'), { items: [], total: 0 });
    const organizationResource = useAsyncResource(async () => apiRequest('/organizations'), { items: [], total: 0 });
    const systemResource = useAsyncResource(async () => apiRequest('/systems'), { items: [], total: 0 });
    const customProtocolResource = useAsyncResource(async () => apiRequest('/custom-protocols'), { items: [], total: 0 });
    const filters = reactive({
      keyword: '',
      organization_id: '',
      system_id: '',
      protocol: '',
      lifecycle_status: '',
      enabled: '',
    });
    const drawerVisible = ref(false);
    const submitting = ref(false);
    const healthLoadingId = ref('');
    const detailVisible = ref(false);
    const detailLoading = ref(false);
    const detail = ref(null);
    const detailStatus = ref(null);
    const detailHealth = ref(null);
    const versionsVisible = ref(false);
    const versionsLoading = ref(false);
    const versionItems = ref([]);
    const versionTargetName = ref('');
    const selectedVersion = ref(null);
    const form = reactive(endpointFormFromItem({}));

    function resetForm(item = {}) {
      Object.assign(form, endpointFormFromItem(item));
    }

    function openCreate() {
      resetForm({});
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    async function save() {
      try {
        const payload = endpointPayloadFromForm(form);
        submitting.value = true;
        if (form.record_id) {
          await apiRequest(`/endpoints/${form.record_id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('端点已更新');
        } else {
          await apiRequest('/endpoints', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('端点已创建');
        }
        drawerVisible.value = false;
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '保存端点失败');
      } finally {
        submitting.value = false;
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除端点 ${item.name || item.id} 吗？`, '删除端点', { type: 'warning' });
        await apiRequest(`/endpoints/${item.id}`, { method: 'DELETE' });
        ElMessage.success('端点已删除');
        await resource.refresh();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除端点失败');
        }
      }
    }

    async function checkHealth(item) {
      try {
        healthLoadingId.value = item.id;
        const result = await apiRequest(`/endpoints/${item.id}/health`);
        ElMessage.success(result.healthy ? '健康检查通过' : '健康检查返回异常');
      } catch (error) {
        ElMessage.error(error.message || '健康检查失败');
      } finally {
        healthLoadingId.value = '';
      }
    }

    async function openDetail(item) {
      try {
        detailLoading.value = true;
        detailVisible.value = true;
        const [endpointResult, statusResult, healthResult] = await Promise.allSettled([
          apiRequest(`/endpoints/${item.id}`),
          apiRequest(`/endpoints/${item.id}/status`),
          apiRequest(`/endpoints/${item.id}/health`),
        ]);

        if (endpointResult.status !== 'fulfilled') {
          throw endpointResult.reason;
        }

        detail.value = endpointResult.value;
        detailStatus.value = statusResult.status === 'fulfilled' ? statusResult.value : endpointResult.value.status || null;
        detailHealth.value = healthResult.status === 'fulfilled' ? healthResult.value : null;
      } catch (error) {
        detailVisible.value = false;
        ElMessage.error(error.message || '加载端点详情失败');
      } finally {
        detailLoading.value = false;
      }
    }

    async function openVersions(item) {
      try {
        versionsLoading.value = true;
        versionsVisible.value = true;
        versionTargetName.value = item.name || item.id;
        const result = await apiRequest(`/endpoints/${item.id}/versions`);
        versionItems.value = result.items || [];
        selectedVersion.value = versionItems.value[0] || null;
      } catch (error) {
        versionsVisible.value = false;
        ElMessage.error(error.message || '加载端点版本历史失败');
      } finally {
        versionsLoading.value = false;
      }
    }

    function selectVersion(item) {
      selectedVersion.value = item || null;
    }

    function fillGeneratedApiKey(fieldName) {
      try {
        form[fieldName] = generateApiKey();
        ElMessage.success('API Key 已生成');
      } catch (error) {
        ElMessage.error(error.message || '生成 API Key 失败');
      }
    }

    async function copyApiKey(fieldName) {
      try {
        await copyTextToClipboard(form[fieldName]);
        ElMessage.success('已复制');
      } catch (error) {
        ElMessage.error(error.message || '复制失败');
      }
    }

    onMounted(async () => {
      await Promise.all([
        resource.refresh(),
        organizationResource.refresh(),
        systemResource.refresh(),
        customProtocolResource.refresh(),
      ]);
    });

    const organizationNameMap = computed(() => {
      const map = new Map();
      (organizationResource.state.value.items || []).forEach((item) => map.set(item.id, item.name || item.id));
      return map;
    });

    const systemNameMap = computed(() => {
      const map = new Map();
      (systemResource.state.value.items || []).forEach((item) => map.set(item.id, item.name || item.id));
      return map;
    });

    const systemTypeMap = computed(() => {
      const map = new Map();
      (systemResource.state.value.items || []).forEach((item) => map.set(item.id, item.system_type || 'OTHER'));
      return map;
    });

    const availableSystems = computed(() => {
      return (systemResource.state.value.items || []).filter((item) => {
        return !form.organization_id || item.organization_id === form.organization_id;
      });
    });

    const filterSystems = computed(() => {
      return (systemResource.state.value.items || []).filter((item) => {
        return !filters.organization_id || item.organization_id === filters.organization_id;
      });
    });

    const filteredItems = computed(() => {
      const keyword = String(filters.keyword || '').trim().toLowerCase();
      return (resource.state.value.items || []).filter((item) => {
        if (filters.organization_id && item.organization_id !== filters.organization_id) {
          return false;
        }
        if (filters.system_id && item.system_id !== filters.system_id) {
          return false;
        }
        if (filters.protocol && item.protocol !== filters.protocol) {
          return false;
        }
        if (filters.lifecycle_status && item.lifecycle_status !== filters.lifecycle_status) {
          return false;
        }
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        if (!keyword) {
          return true;
        }
        const haystack = [
          item.id,
          item.name,
          item.description,
          organizationNameMap.value.get(item.organization_id),
          systemNameMap.value.get(item.system_id),
          buildConnectionAddress(item.connection, item.protocol),
          ...(item.roles || []),
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase();
        return haystack.includes(keyword);
      });
    });
    const pager = usePagedList(filteredItems, { reservedHeight: 470, compactReservedHeight: 410 });

    watch(() => form.organization_id, (organizationId) => {
      if (!form.system_id) {
        return;
      }
      const current = (systemResource.state.value.items || []).find((item) => item.id === form.system_id);
      if (current && organizationId && current.organization_id !== organizationId) {
        form.system_id = '';
      }
    });

    watch(() => form.system_id, (systemId) => {
      const selected = (systemResource.state.value.items || []).find((item) => item.id === systemId);
      if (!selected) {
        return;
      }
      form.organization_id = selected.organization_id;
      form.system_type = selected.system_type;
    });

    watch(() => form.protocol, (protocol) => {
      if (protocol !== 'CUSTOM') {
        form.custom_protocol_id = '';
      }
      if (protocol === 'DATABASE') {
        form.port = defaultPortByDatabaseType(form.database_type);
      }
      if (protocol === 'OPENAI') {
        form.host = form.host || 'api.openai.com';
        form.port = 443;
        form.tls_enabled = true;
        form.path = defaultPathByOpenAiEndpoint(form.openai_endpoint_type);
        if (form.auth_type === 'none') {
          form.auth_type = 'bearer';
        }
      }
      if (protocol === 'WEBHOOK') {
        form.port = defaultPortByProtocol(protocol, form.tls_enabled);
        form.path = form.path || '/webhook';
      }
    });

    watch(() => form.database_type, (databaseType) => {
      if (form.protocol === 'DATABASE') {
        form.port = defaultPortByDatabaseType(databaseType);
      }
    });

    watch(() => form.openai_endpoint_type, (endpointType) => {
      if (form.protocol === 'OPENAI') {
        form.path = defaultPathByOpenAiEndpoint(endpointType);
      }
    });

    watch(() => filters.organization_id, (organizationId) => {
      if (!filters.system_id) {
        return;
      }
      const selected = (systemResource.state.value.items || []).find((item) => item.id === filters.system_id);
      if (selected && organizationId && selected.organization_id !== organizationId) {
        filters.system_id = '';
      }
    });

    function resetFilters() {
      Object.assign(filters, {
        keyword: '',
        organization_id: '',
        system_id: '',
        protocol: '',
        lifecycle_status: '',
        enabled: '',
      });
    }

    return {
      ...resource,
      ...pager,
      organizationResource,
      systemResource,
      customProtocolResource,
      filters,
      drawerVisible,
      form,
      submitting,
      healthLoadingId,
      detailVisible,
      detailLoading,
      detail,
      detailStatus,
      detailHealth,
      versionsVisible,
      versionsLoading,
      versionItems,
      versionTargetName,
      selectedVersion,
      protocolOptions,
      systemTypeOptions,
      endpointRoleOptions,
      endpointLifecycleOptions,
      endpointEncryptionOptions,
      authTypeOptions,
      databaseTypeOptions,
      openAiEndpointOptions,
      webhookMethodOptions,
      organizationNameMap,
      systemNameMap,
      systemTypeMap,
      availableSystems,
      filterSystems,
      filteredItems,
      optionLabel,
      formatTime,
      buildConnectionAddress,
      openCreate,
      openEdit,
      openDetail,
      openVersions,
      selectVersion,
      fillGeneratedApiKey,
      copyApiKey,
      save,
      remove,
      checkHealth,
      resetFilters,
      stringifyPretty,
      tagTypeByState,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">端点管理</h2>
            <p class="page-section-subtitle">端点现在显式属于系统并间接属于机构。这里维护物理连接、角色、加密算法和认证策略。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建端点</el-button>
          </div>
        </div>

        <el-alert v-if="state.error" :title="state.error" type="error" show-icon :closable="false" style="margin-bottom: 16px;" />

        <div class="list-filter-bar">
          <div class="filters-grid">
          <el-form-item label="关键字">
            <el-input v-model="filters.keyword" placeholder="搜索名称、ID、机构、系统、地址" clearable />
          </el-form-item>
          <el-form-item label="机构">
            <el-select v-model="filters.organization_id" filterable clearable placeholder="全部机构">
              <el-option
                v-for="item in organizationResource.state.value.items || []"
                :key="item.id"
                :label="item.name"
                :value="item.id"
              />
            </el-select>
          </el-form-item>
          <el-form-item label="系统">
            <el-select v-model="filters.system_id" filterable clearable placeholder="全部系统">
              <el-option v-for="item in filterSystems" :key="item.id" :label="item.name" :value="item.id" />
            </el-select>
          </el-form-item>
          <el-form-item label="协议">
            <el-select v-model="filters.protocol" clearable>
              <el-option v-for="item in protocolOptions" :key="item" :label="item" :value="item" />
            </el-select>
          </el-form-item>
          <el-form-item label="生命周期">
            <el-select v-model="filters.lifecycle_status" clearable>
              <el-option v-for="item in endpointLifecycleOptions" :key="item.value" :label="item.label" :value="item.value" />
            </el-select>
          </el-form-item>
          <el-form-item label="启用状态">
            <el-select v-model="filters.enabled" clearable>
              <el-option label="启用" value="true" />
              <el-option label="停用" value="false" />
            </el-select>
          </el-form-item>
          </div>
        </div>

        <el-table :data="pagedItems" stripe v-loading="state.loading || organizationResource.state.loading || systemResource.state.loading" row-key="id" class="list-table" :height="tableHeight">
          <el-table-column prop="name" label="名称" min-width="180" />
          <el-table-column label="所属机构" min-width="160">
            <template #default="scope">{{ organizationNameMap.get(scope.row.organization_id) || scope.row.organization_id || '-' }}</template>
          </el-table-column>
          <el-table-column label="所属系统" min-width="160">
            <template #default="scope">{{ systemNameMap.get(scope.row.system_id) || scope.row.system_id || '-' }}</template>
          </el-table-column>
          <el-table-column prop="protocol" label="协议" width="120" />
          <el-table-column label="角色" min-width="180">
            <template #default="scope">
              <el-space wrap>
                <el-tag v-for="role in scope.row.roles || []" :key="role" effect="plain">{{ optionLabel(endpointRoleOptions, role) }}</el-tag>
              </el-space>
            </template>
          </el-table-column>
          <el-table-column label="地址" min-width="260">
            <template #default="scope">{{ buildConnectionAddress(scope.row.connection, scope.row.protocol) }}</template>
          </el-table-column>
          <el-table-column label="状态" width="160">
            <template #default="scope">
              <el-space wrap>
                <el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag>
                <el-tag :type="tagTypeByState(scope.row.lifecycle_status)">{{ scope.row.lifecycle_status }}</el-tag>
              </el-space>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="280" fixed="right">
            <template #default="scope">
              <el-space wrap>
                <el-button link type="info" @click="openDetail(scope.row)">详情</el-button>
                <el-button link type="warning" @click="openVersions(scope.row)">版本</el-button>
                <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
                <el-button link type="success" :loading="healthLoadingId === scope.row.id" @click="checkHealth(scope.row)">健康检查</el-button>
                <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="currentPage" background layout="pager" :page-size="pageSize" :pager-count="7" :total="total" />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.record_id ? '编辑端点' : '新建端点'" size="820px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item v-if="!form.record_id" label="端点ID">
              <el-input v-model="form.requested_id" placeholder="可选，留空则后端自动生成" />
            </el-form-item>
            <el-form-item label="所属机构">
              <el-select v-model="form.organization_id" filterable clearable placeholder="先选机构，再选系统">
                <el-option
                  v-for="item in organizationResource.state.value.items || []"
                  :key="item.id"
                  :label="item.name"
                  :value="item.id"
                />
              </el-select>
            </el-form-item>
            <el-form-item label="所属系统">
              <el-select v-model="form.system_id" filterable placeholder="请选择系统">
                <el-option v-for="item in availableSystems" :key="item.id" :label="item.name" :value="item.id" />
              </el-select>
            </el-form-item>
            <el-form-item label="端点名称"><el-input v-model="form.name" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="协议">
              <el-select v-model="form.protocol">
                <el-option v-for="item in protocolOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item v-if="form.protocol === 'CUSTOM'" label="自定义协议">
              <el-select v-model="form.custom_protocol_id" filterable placeholder="请选择已启用的自定义协议">
                <el-option
                  v-for="item in (customProtocolResource.state.value.items || []).filter((protocol) => protocol.enabled)"
                  :key="item.id"
                  :label="item.name || item.id"
                  :value="item.id"
                />
              </el-select>
            </el-form-item>
            <el-form-item v-if="form.protocol === 'DATABASE'" label="数据库类型">
              <el-select v-model="form.database_type" filterable>
                <el-option v-for="item in databaseTypeOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item v-if="form.protocol === 'OPENAI'" label="OpenAI 接口">
              <el-select v-model="form.openai_endpoint_type">
                <el-option v-for="item in openAiEndpointOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item v-if="form.protocol === 'WEBHOOK'" label="Webhook 方法">
              <el-select v-model="form.webhook_method">
                <el-option v-for="item in webhookMethodOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item label="系统类型">
              <el-input v-model="form.system_type" disabled />
            </el-form-item>
            <el-form-item label="生命周期">
              <el-select v-model="form.lifecycle_status">
                <el-option v-for="item in endpointLifecycleOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" />
            </el-form-item>
          </div>

          <el-form-item label="端点角色">
            <el-checkbox-group v-model="form.roles">
              <el-checkbox v-for="item in endpointRoleOptions" :key="item.value" :label="item.value">{{ item.label }}</el-checkbox>
            </el-checkbox-group>
          </el-form-item>

          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="2" /></el-form-item>

          <div class="filters-grid" v-if="form.protocol === 'DATABASE'">
            <el-form-item label="数据库名"><el-input v-model="form.database_name" placeholder="hsb 或 service name" /></el-form-item>
            <el-form-item label="Schema"><el-input v-model="form.database_schema" placeholder="public、dbo 等" /></el-form-item>
            <el-form-item label="JDBC URL"><el-input v-model="form.jdbc_url" placeholder="可选，用于记录标准连接串" /></el-form-item>
          </div>

          <div class="filters-grid" v-if="form.protocol === 'OPENAI'">
            <el-form-item label="默认模型"><el-input v-model="form.openai_model" placeholder="gpt-4o-mini" /></el-form-item>
            <el-form-item label="Organization"><el-input v-model="form.openai_organization" placeholder="可选" /></el-form-item>
            <el-form-item label="Project"><el-input v-model="form.openai_project" placeholder="可选" /></el-form-item>
          </div>

          <div class="filters-grid" v-if="form.protocol === 'WEBHOOK'">
            <el-form-item label="事件类型"><el-input v-model="form.webhook_event" placeholder="patient.admit、order.created 等" /></el-form-item>
            <el-form-item label="签名 Header"><el-input v-model="form.webhook_signature_header" placeholder="X-HSB-Signature" /></el-form-item>
            <el-form-item label="签名密钥引用">
              <el-input v-model="form.webhook_signing_secret_ref" class="api-key-input" placeholder="secret/webhook/lis-main，可选">
                <template #append>
                  <el-button @click="fillGeneratedApiKey('webhook_signing_secret_ref')">随机生成api-key</el-button>
                  <el-button @click="copyApiKey('webhook_signing_secret_ref')">复制</el-button>
                </template>
              </el-input>
            </el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="主机"><el-input v-model="form.host" placeholder="his-api.internal" /></el-form-item>
            <el-form-item label="端口"><el-input-number v-model="form.port" :min="1" :max="65535" style="width: 100%;" /></el-form-item>
            <el-form-item label="路径"><el-input v-model="form.path" placeholder="/api/v1/messages" /></el-form-item>
            <el-form-item label="TLS 证书路径"><el-input v-model="form.tls_cert_path" placeholder="可选" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="启用 TLS"><el-switch v-model="form.tls_enabled" /></el-form-item>
            <el-form-item label="连接超时(秒)"><el-input-number v-model="form.connect_timeout_secs" :min="1" style="width: 100%;" /></el-form-item>
            <el-form-item label="读取超时(秒)"><el-input-number v-model="form.read_timeout_secs" :min="1" style="width: 100%;" /></el-form-item>
            <el-form-item label="写入超时(秒)"><el-input-number v-model="form.write_timeout_secs" :min="1" style="width: 100%;" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="连接池大小"><el-input-number v-model="form.pool_size" :min="1" style="width: 100%;" /></el-form-item>
            <el-form-item label="重连间隔(秒)"><el-input-number v-model="form.reconnect_interval_secs" :min="1" style="width: 100%;" /></el-form-item>
            <el-form-item label="Keepalive(秒)"><el-input-number v-model="form.keepalive_secs" :min="0" style="width: 100%;" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="最大重试次数"><el-input-number v-model="form.max_retries" :min="0" style="width: 100%;" /></el-form-item>
            <el-form-item label="重试间隔(ms)"><el-input-number v-model="form.retry_interval_ms" :min="0" :step="100" style="width: 100%;" /></el-form-item>
            <el-form-item label="最大消息大小"><el-input-number v-model="form.max_message_size" :min="1024" :step="1024" style="width: 100%;" /></el-form-item>
            <el-form-item label="并发限制"><el-input-number v-model="form.concurrency_limit" :min="1" style="width: 100%;" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="速率限制/秒"><el-input-number v-model="form.rate_limit" :min="0" style="width: 100%;" /></el-form-item>
            <el-form-item label="熔断阈值"><el-input-number v-model="form.circuit_breaker_threshold" :min="0" style="width: 100%;" /></el-form-item>
            <el-form-item label="压缩"><el-switch v-model="form.compression_enabled" /></el-form-item>
            <el-form-item label="记录报文体"><el-switch v-model="form.log_body" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="认证类型">
              <el-select v-model="form.auth_type">
                <el-option v-for="item in authTypeOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item label="密钥引用">
              <el-input v-model="form.security_secret_ref" class="api-key-input" placeholder="vault://hsb/endpoints/demo">
                <template #append>
                  <el-button @click="fillGeneratedApiKey('security_secret_ref')">随机生成api-key</el-button>
                  <el-button @click="copyApiKey('security_secret_ref')">复制</el-button>
                </template>
              </el-input>
            </el-form-item>
            <el-form-item label="加密算法">
              <el-select v-model="form.security_encryption_algorithm">
                <el-option v-for="item in endpointEncryptionOptions" :key="item.value" :label="item.label" :value="item.value" />
              </el-select>
            </el-form-item>
            <el-form-item label="凭证过期时间">
              <el-input v-model="form.security_credential_expires_at" placeholder="ISO8601，可选" />
            </el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="强制 TLS"><el-switch v-model="form.security_require_tls" /></el-form-item>
            <el-form-item label="跳过证书校验"><el-switch v-model="form.security_allow_insecure_skip_verify" /></el-form-item>
            <el-form-item label="日志脱敏"><el-switch v-model="form.security_mask_credentials_in_logs" /></el-form-item>
          </div>

          <el-form-item label="允许 IP 段">
            <el-input v-model="form.security_allowed_ip_ranges_text" type="textarea" :rows="3" placeholder="每行一个 CIDR，也可使用逗号分隔" />
          </el-form-item>

          <el-alert
            v-if="form.record_id && form.auth_preserve_existing && form.auth_type !== 'none'"
            title="当前端点已存在认证配置；如果密文字段留空，将沿用后端已保存的凭据。"
            type="info"
            show-icon
            :closable="false"
            style="margin-bottom: 16px;"
          />

          <div class="filters-grid" v-if="form.auth_type === 'basic'">
            <el-form-item label="用户名"><el-input v-model="form.auth_username" /></el-form-item>
            <el-form-item label="密码"><el-input v-model="form.auth_password" type="password" show-password placeholder="编辑时留空表示不变" /></el-form-item>
          </div>
          <div class="filters-grid" v-if="form.auth_type === 'bearer'">
            <el-form-item label="Token"><el-input v-model="form.auth_token" type="password" show-password placeholder="编辑时留空表示不变" /></el-form-item>
          </div>
          <div class="filters-grid" v-if="form.auth_type === 'api_key'">
            <el-form-item label="Header 名称"><el-input v-model="form.auth_header_name" placeholder="X-API-Key" /></el-form-item>
            <el-form-item label="API Key">
              <el-input v-model="form.auth_api_secret" class="api-key-input" type="password" show-password placeholder="编辑时留空表示不变">
                <template #append>
                  <el-button @click="fillGeneratedApiKey('auth_api_secret')">随机生成api-key</el-button>
                  <el-button @click="copyApiKey('auth_api_secret')">复制</el-button>
                </template>
              </el-input>
            </el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="操作人"><el-input v-model="form.actor" placeholder="ops-user" /></el-form-item>
            <el-form-item label="变更说明"><el-input v-model="form.change_note" placeholder="例如：切换为新网关地址" /></el-form-item>
          </div>

          <el-form-item label="属性 JSON" class="code-block"><el-input v-model="form.propertiesText" type="textarea" /></el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>

      <el-dialog v-model="detailVisible" title="端点详情" width="980px">
        <div v-loading="detailLoading">
          <el-descriptions v-if="detail" :column="2" border>
            <el-descriptions-item label="端点ID">{{ detail.id }}</el-descriptions-item>
            <el-descriptions-item label="名称">{{ detail.name }}</el-descriptions-item>
            <el-descriptions-item label="所属机构">{{ organizationNameMap.get(detail.organization_id) || detail.organization_id || '-' }}</el-descriptions-item>
            <el-descriptions-item label="所属系统">{{ systemNameMap.get(detail.system_id) || detail.system_id || '-' }}</el-descriptions-item>
            <el-descriptions-item label="系统类型">{{ detail.system_type }}</el-descriptions-item>
            <el-descriptions-item label="协议">{{ detail.protocol }}</el-descriptions-item>
            <el-descriptions-item label="连接地址">{{ buildConnectionAddress(detail.connection, detail.protocol) }}</el-descriptions-item>
            <el-descriptions-item label="版本">v{{ detail.version }}</el-descriptions-item>
            <el-descriptions-item label="生命周期">
              <el-tag :type="tagTypeByState(detail.lifecycle_status)">{{ detail.lifecycle_status }}</el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="启用状态">
              <el-tag :type="detail.enabled ? 'success' : 'info'">{{ detail.enabled ? '启用' : '停用' }}</el-tag>
            </el-descriptions-item>
            <el-descriptions-item label="角色">
              <el-space wrap>
                <el-tag v-for="role in detail.roles || []" :key="role" effect="plain">{{ optionLabel(endpointRoleOptions, role) }}</el-tag>
              </el-space>
            </el-descriptions-item>
            <el-descriptions-item label="认证摘要">{{ detail.auth?.auth_type || 'none' }}</el-descriptions-item>
            <el-descriptions-item label="主责人">{{ detail.updated_by || detail.created_by || '-' }}</el-descriptions-item>
            <el-descriptions-item label="创建时间">{{ formatTime(detail.created_at) }}</el-descriptions-item>
            <el-descriptions-item label="更新时间">{{ formatTime(detail.updated_at) }}</el-descriptions-item>
            <el-descriptions-item label="描述" :span="2">{{ detail.description || '-' }}</el-descriptions-item>
          </el-descriptions>

          <div class="filters-grid" style="margin-top: 16px;">
            <div class="page-card" style="padding: 16px;">
              <h3 class="page-section-title" style="font-size: 16px; margin-bottom: 12px;">运行状态</h3>
              <el-descriptions :column="1" border>
                <el-descriptions-item label="健康">{{ detailStatus?.healthy ? '健康' : '异常' }}</el-descriptions-item>
                <el-descriptions-item label="延迟(ms)">{{ detailStatus?.latency_ms ?? '-' }}</el-descriptions-item>
                <el-descriptions-item label="熔断器">{{ detailStatus?.circuit_state || '-' }}</el-descriptions-item>
                <el-descriptions-item label="连续失败">{{ detailStatus?.consecutive_failures ?? 0 }}</el-descriptions-item>
                <el-descriptions-item label="最近检查">{{ formatTime(detailStatus?.last_check_at) }}</el-descriptions-item>
                <el-descriptions-item label="最近投递">{{ formatTime(detailStatus?.last_delivery_at) }}</el-descriptions-item>
                <el-descriptions-item label="最近错误">{{ detailStatus?.last_error || '-' }}</el-descriptions-item>
              </el-descriptions>
            </div>

            <div class="page-card" style="padding: 16px;">
              <h3 class="page-section-title" style="font-size: 16px; margin-bottom: 12px;">安全视图</h3>
              <el-descriptions :column="1" border>
                <el-descriptions-item label="密钥引用">{{ detail?.security?.secret_ref || '-' }}</el-descriptions-item>
                <el-descriptions-item label="加密算法">{{ optionLabel(endpointEncryptionOptions, detail?.security?.encryption_algorithm) }}</el-descriptions-item>
                <el-descriptions-item label="强制 TLS">{{ detail?.security?.require_tls ? '是' : '否' }}</el-descriptions-item>
                <el-descriptions-item label="允许跳过校验">{{ detail?.security?.allow_insecure_skip_verify ? '是' : '否' }}</el-descriptions-item>
                <el-descriptions-item label="健康检查">{{ detailHealth?.healthy ? '通过' : (detailHealth ? '异常' : '-') }}</el-descriptions-item>
                <el-descriptions-item label="检查时间">{{ formatTime(detailHealth?.checked_at) }}</el-descriptions-item>
                <el-descriptions-item label="允许 IP 段">{{ (detail?.security?.allowed_ip_ranges || []).join(', ') || '-' }}</el-descriptions-item>
              </el-descriptions>
            </div>
          </div>

          <el-form-item label="连接配置 JSON" class="code-block" style="margin-top: 16px;">
            <el-input :model-value="stringifyPretty(detail?.connection || {})" type="textarea" readonly />
          </el-form-item>
          <el-form-item label="端点配置 JSON" class="code-block">
            <el-input :model-value="stringifyPretty(detail?.config || {})" type="textarea" readonly />
          </el-form-item>
          <el-form-item label="属性 JSON" class="code-block">
            <el-input :model-value="stringifyPretty(detail?.properties || {})" type="textarea" readonly />
          </el-form-item>
        </div>
      </el-dialog>

      <el-drawer v-model="versionsVisible" :title="versionTargetName ? versionTargetName + ' - 版本历史' : '版本历史'" size="860px">
        <el-table :data="versionItems" stripe highlight-current-row v-loading="versionsLoading" @current-change="selectVersion">
          <el-table-column prop="version" label="版本" width="90">
            <template #default="scope">v{{ scope.row.version }}</template>
          </el-table-column>
          <el-table-column label="变更时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.changed_at) }}</template>
          </el-table-column>
          <el-table-column prop="changed_by" label="操作人" width="160" />
          <el-table-column prop="change_note" label="变更说明" min-width="220" />
          <el-table-column label="快照摘要" min-width="260">
            <template #default="scope">
              {{ scope.row.snapshot?.protocol || '-' }} / {{ scope.row.snapshot?.name || '-' }} / {{ buildConnectionAddress(scope.row.snapshot?.connection, scope.row.snapshot?.protocol) }}
            </template>
          </el-table-column>
        </el-table>

        <el-empty v-if="!versionsLoading && !versionItems.length" description="暂无版本历史" />

        <el-form-item v-if="selectedVersion" :label="'版本 v' + selectedVersion.version + ' 快照 JSON'" class="code-block" style="margin-top: 16px;">
          <el-input :model-value="stringifyPretty(selectedVersion?.snapshot || {})" type="textarea" readonly />
        </el-form-item>
      </el-drawer>
    </div>
  `,
};

const CustomProtocolsPage = {
  name: 'CustomProtocolsPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/custom-protocols'), { items: [], total: 0 });
    const drawerVisible = ref(false);
    const submitting = ref(false);
    const filters = reactive({ keyword: '', transport_hint: '', content_type: '', enabled: '' });
    const form = reactive(customProtocolFormFromItem({}));

    const transportHintOptions = computed(() => uniqueValues(resource.state.value.items || [], 'transport_hint'));
    const contentTypeOptions = computed(() => uniqueValues(resource.state.value.items || [], 'content_type'));
    const filteredItems = computed(() => {
      return (resource.state.value.items || []).filter((item) => {
        if (filters.transport_hint && item.transport_hint !== filters.transport_hint) {
          return false;
        }
        if (filters.content_type && item.content_type !== filters.content_type) {
          return false;
        }
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        return matchesKeyword(item, filters.keyword, ['id', 'name', 'description', 'transport_hint', 'content_type']);
      });
    });
    const pager = usePagedList(filteredItems);

    function resetForm(item = {}) {
      Object.assign(form, customProtocolFormFromItem(item));
    }

    function openCreate() {
      resetForm({});
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    async function save() {
      try {
        submitting.value = true;
        const isUpdate = !!form.id;
        const payload = customProtocolPayloadFromForm(form, isUpdate);
        if (isUpdate) {
          await apiRequest(`/custom-protocols/${form.id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('自定义协议已更新');
        } else {
          await apiRequest('/custom-protocols', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('自定义协议已创建');
        }
        drawerVisible.value = false;
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '保存自定义协议失败');
      } finally {
        submitting.value = false;
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除自定义协议 ${item.name || item.id} 吗？`, '删除自定义协议', { type: 'warning' });
        await apiRequest(`/custom-protocols/${item.id}`, { method: 'DELETE' });
        ElMessage.success('自定义协议已删除');
        await resource.refresh();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除自定义协议失败');
        }
      }
    }

    onMounted(resource.refresh);

    function resetFilters() {
      Object.assign(filters, { keyword: '', transport_hint: '', content_type: '', enabled: '' });
    }

    return {
      ...resource,
      ...pager,
      drawerVisible,
      submitting,
      filters,
      form,
      transportHintOptions,
      contentTypeOptions,
      filteredItems,
      openCreate,
      openEdit,
      save,
      remove,
      resetFilters,
      stringifyPretty,
      formatTime,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">自定义协议</h2>
            <p class="page-section-subtitle">维护 CUSTOM 协议的字段结构，端点选择 CUSTOM 时必须引用这里的协议定义。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建协议</el-button>
          </div>
        </div>
        <el-alert v-if="state.error" :title="state.error" type="error" show-icon :closable="false" style="margin-bottom: 16px;" />
        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入协议名称、ID 或描述" clearable />
            </el-form-item>
            <el-form-item label="传输提示">
              <el-select v-model="filters.transport_hint" clearable placeholder="全部传输">
                <el-option v-for="item in transportHintOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="Content-Type">
              <el-select v-model="filters.content_type" clearable placeholder="全部类型">
                <el-option v-for="item in contentTypeOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-select v-model="filters.enabled" clearable placeholder="全部状态">
                <el-option label="启用" value="true" />
                <el-option label="停用" value="false" />
              </el-select>
            </el-form-item>
          </div>
        </div>
        <el-table :data="pagedItems" stripe v-loading="state.loading" row-key="id" class="list-table" :height="tableHeight">
          <el-table-column prop="id" label="协议ID" min-width="180" />
          <el-table-column prop="name" label="名称" min-width="180" />
          <el-table-column prop="transport_hint" label="传输提示" width="140" />
          <el-table-column prop="content_type" label="Content-Type" min-width="160" />
          <el-table-column label="字段数" width="100"><template #default="scope">{{ (scope.row.fields || []).length }}</template></el-table-column>
          <el-table-column label="状态" width="100"><template #default="scope"><el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag></template></el-table-column>
          <el-table-column label="更新时间" width="180"><template #default="scope">{{ formatTime(scope.row.updated_at) }}</template></el-table-column>
          <el-table-column label="操作" width="150" fixed="right">
            <template #default="scope">
              <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
              <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="currentPage" background layout="pager" :page-size="pageSize" :pager-count="7" :total="total" />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.id ? '编辑自定义协议' : '新建自定义协议'" size="820px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item v-if="!form.id" label="协议ID"><el-input v-model="form.requested_id" placeholder="可选，留空则自动生成" /></el-form-item>
            <el-form-item label="名称"><el-input v-model="form.name" /></el-form-item>
            <el-form-item label="传输提示">
              <el-select v-model="form.transport_hint" placeholder="请选择传输类型" allow-create filterable clearable style="width:100%">
                <el-option label="HTTP" value="http" />
                <el-option label="TCP / MLLP" value="tcp" />
                <el-option label="MQ (AMQP)" value="mq" />
                <el-option label="Kafka" value="kafka" />
                <el-option label="NATS" value="nats" />
                <el-option label="gRPC" value="grpc" />
              </el-select>
            </el-form-item>
            <el-form-item label="Content-Type"><el-input v-model="form.content_type" placeholder="application/json" /></el-form-item>
            <el-form-item label="启用状态"><el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" /></el-form-item>
          </div>
          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="2" /></el-form-item>
          <el-form-item label="字段定义 JSON" class="code-block"><el-input v-model="form.fieldsText" type="textarea" :rows="14" /></el-form-item>
          <el-form-item label="样例报文 JSON" class="code-block"><el-input v-model="form.samplePayloadText" type="textarea" :rows="10" /></el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>
    </div>
  `,
};

const TopicsPage = {
  name: 'TopicsPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/topics'), { items: [], total: 0 });
    const systemResource = useAsyncResource(async () => apiRequest('/systems'), { items: [], total: 0 });
    const drawerVisible = ref(false);
    const submitting = ref(false);
    const filters = reactive({ keyword: '', domain: '', service: '', owner_system_id: '', enabled: '' });
    const form = reactive(topicFormFromItem({}));

    const topicItems = computed(() => resource.state.value.items || []);
    const domainOptions = computed(() => uniqueValues(topicItems.value, 'domain'));
    const serviceOptions = computed(() => uniqueValues(
      topicItems.value.filter((item) => !filters.domain || item.domain === filters.domain),
      'service',
    ));
    const systemNameMap = computed(() => {
      const map = new Map();
      (systemResource.state.value.items || []).forEach((item) => map.set(item.id, item.name || item.id));
      return map;
    });
    const filteredItems = computed(() => {
      return topicItems.value.filter((item) => {
        if (filters.domain && item.domain !== filters.domain) {
          return false;
        }
        if (filters.service && item.service !== filters.service) {
          return false;
        }
        if (filters.owner_system_id && item.owner_system_id !== filters.owner_system_id) {
          return false;
        }
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        return matchesKeyword(item, filters.keyword, [
          'topic',
          'description',
          'domain',
          'service',
          'action',
          'version',
          (value) => systemNameMap.value.get(value.owner_system_id),
        ]);
      });
    });
    const pager = usePagedList(filteredItems);

    function resetForm(item = {}) {
      Object.assign(form, topicFormFromItem(item));
    }

    function openCreate() {
      resetForm({});
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    async function save() {
      try {
        submitting.value = true;
        const payload = topicPayloadFromForm(form);
        if (form.id) {
          await apiRequest(`/topics/${encodeURIComponent(form.id)}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('Topic 已更新');
        } else {
          await apiRequest('/topics', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('Topic 已创建');
        }
        drawerVisible.value = false;
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '保存 Topic 失败');
      } finally {
        submitting.value = false;
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除 Topic ${item.topic} 吗？`, '删除 Topic', { type: 'warning' });
        await apiRequest(`/topics/${encodeURIComponent(item.id)}`, { method: 'DELETE' });
        ElMessage.success('Topic 已删除');
        await resource.refresh();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除 Topic 失败');
        }
      }
    }

    onMounted(async () => {
      await Promise.all([resource.refresh(), systemResource.refresh()]);
    });

    watch(() => filters.domain, () => {
      filters.service = '';
    });

    function resetFilters() {
      Object.assign(filters, { keyword: '', domain: '', service: '', owner_system_id: '', enabled: '' });
    }

    return {
      ...resource,
      ...pager,
      systemResource,
      drawerVisible,
      submitting,
      filters,
      domainOptions,
      serviceOptions,
      systemNameMap,
      filteredItems,
      form,
      openCreate,
      openEdit,
      save,
      remove,
      resetFilters,
      formatTime,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">Topic 目录</h2>
            <p class="page-section-subtitle">维护消息主题命名与归属，格式为 domain.service.action.version。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建 Topic</el-button>
          </div>
        </div>
        <el-alert v-if="state.error" :title="state.error" type="error" show-icon :closable="false" style="margin-bottom: 16px;" />
        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入 Topic 名、说明、Action" clearable />
            </el-form-item>
            <el-form-item label="Domain">
              <el-select v-model="filters.domain" clearable placeholder="全部 Domain">
                <el-option v-for="item in domainOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="Service">
              <el-select v-model="filters.service" clearable placeholder="全部 Service">
                <el-option v-for="item in serviceOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="归属系统">
              <el-select v-model="filters.owner_system_id" filterable clearable placeholder="全部系统">
                <el-option v-for="item in systemResource.state.value.items || []" :key="item.id" :label="item.name" :value="item.id" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-select v-model="filters.enabled" clearable placeholder="全部状态">
                <el-option label="启用" value="true" />
                <el-option label="停用" value="false" />
              </el-select>
            </el-form-item>
          </div>
        </div>
        <el-table :data="pagedItems" stripe v-loading="state.loading" row-key="id" class="list-table topic-table" :height="tableHeight">
          <el-table-column prop="topic" label="Topic" min-width="240" />
          <el-table-column label="说明" min-width="360" show-overflow-tooltip>
            <template #default="scope"><span class="topic-description">{{ scope.row.description || '-' }}</span></template>
          </el-table-column>
          <el-table-column prop="domain" label="Domain" width="120" />
          <el-table-column prop="service" label="Service" width="140" />
          <el-table-column prop="action" label="Action" width="140" />
          <el-table-column prop="version" label="版本" width="100" />
          <el-table-column label="状态" width="100"><template #default="scope"><el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag></template></el-table-column>
          <el-table-column label="更新时间" width="180"><template #default="scope">{{ formatTime(scope.row.updated_at) }}</template></el-table-column>
          <el-table-column label="操作" width="150" fixed="right">
            <template #default="scope">
              <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
              <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination
            v-model:current-page="currentPage"
            background
            layout="pager"
            :page-size="pageSize"
            :pager-count="7"
            :total="total"
          />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.id ? '编辑 Topic' : '新建 Topic'" size="720px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item label="Topic"><el-input v-model="form.topic" placeholder="hsb.events.ingress.v1" :disabled="!!form.id" /></el-form-item>
            <el-form-item label="归属系统">
              <el-select v-model="form.owner_system_id" filterable clearable placeholder="可选">
                <el-option v-for="item in systemResource.state.value.items || []" :key="item.id" :label="item.name" :value="item.id" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态"><el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" /></el-form-item>
          </div>
          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="2" /></el-form-item>
          <el-form-item label="属性 JSON" class="code-block"><el-input v-model="form.propertiesText" type="textarea" /></el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>
    </div>
  `,
};

const RoutesPage = {
  name: 'RoutesPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/routes'), { items: [], total: 0 });
    const endpointResource = useAsyncResource(async () => apiRequest('/endpoints'), { items: [], total: 0 });
    const drawerVisible = ref(false);
    const submitting = ref(false);
    const filters = reactive({ keyword: '', system_type: '', protocol: '', enabled: '' });
    const form = reactive(routeFormFromItem({}));

    function resetForm(item = {}) {
      Object.assign(form, routeFormFromItem(item));
    }

    function openCreate() {
      resetForm({});
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    async function save() {
      try {
        submitting.value = true;
        const payload = routePayloadFromForm(form);
        if (form.id) {
          await apiRequest(`/routes/${form.id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('路由已更新');
        } else {
          await apiRequest('/routes', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('路由已创建');
        }
        drawerVisible.value = false;
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '保存路由失败');
      } finally {
        submitting.value = false;
      }
    }

    async function toggleRoute(item, enabled) {
      try {
        await apiRequest(`/routes/${item.id}/${enabled ? 'enable' : 'disable'}`, { method: 'POST' });
        ElMessage.success(enabled ? '路由已启用' : '路由已停用');
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '更新路由状态失败');
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除路由 ${item.name || item.id} 吗？`, '删除路由', { type: 'warning' });
        await apiRequest(`/routes/${item.id}`, { method: 'DELETE' });
        ElMessage.success('路由已删除');
        await resource.refresh();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除路由失败');
        }
      }
    }

    onMounted(async () => {
      await Promise.all([resource.refresh(), endpointResource.refresh()]);
    });

    const endpointNames = computed(() => {
      const map = new Map();
      const items = endpointResource.state.value.items || [];
      items.forEach((item) => map.set(item.id, item.name || item.id));
      return map;
    });

    const filteredItems = computed(() => {
      return (resource.state.value.items || []).filter((item) => {
        if (filters.system_type && item.source?.system_type !== filters.system_type) {
          return false;
        }
        if (filters.protocol && item.source?.protocol !== filters.protocol) {
          return false;
        }
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        return matchesKeyword(item, filters.keyword, [
          'id',
          'name',
          'description',
          (value) => value.source?.endpoint_id,
          (value) => value.source?.pattern,
          (value) => value.source?.message_type,
          (value) => (value.targets || []).map((target) => [target.endpoint_id, endpointNames.value.get(target.endpoint_id)]),
        ]);
      });
    });
    const pager = usePagedList(filteredItems);

    function resetFilters() {
      Object.assign(filters, { keyword: '', system_type: '', protocol: '', enabled: '' });
    }

    return {
      ...resource,
      ...pager,
      endpointResource,
      drawerVisible,
      submitting,
      filters,
      form,
      protocolOptions,
      systemTypeOptions,
      routePriorityOptions,
      endpointNames,
      filteredItems,
      openCreate,
      openEdit,
      save,
      toggleRoute,
      remove,
      resetFilters,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">路由管理</h2>
            <p class="page-section-subtitle">定义统一消息进入后如何根据来源、协议、消息类型转发到目标端点。这里维护的是逻辑选择规则，不直接绑定物理地址。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建路由</el-button>
          </div>
        </div>

        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入路由名称、ID、来源或目标端点" clearable />
            </el-form-item>
            <el-form-item label="来源系统类型">
              <el-select v-model="filters.system_type" clearable placeholder="全部类型">
                <el-option v-for="item in systemTypeOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="协议">
              <el-select v-model="filters.protocol" clearable placeholder="全部协议">
                <el-option v-for="item in protocolOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-select v-model="filters.enabled" clearable placeholder="全部状态">
                <el-option label="启用" value="true" />
                <el-option label="停用" value="false" />
              </el-select>
            </el-form-item>
          </div>
        </div>

        <el-table :data="pagedItems" stripe v-loading="state.loading" row-key="id" class="list-table" :height="tableHeight">
          <el-table-column prop="name" label="名称" min-width="180" />
          <el-table-column prop="priority" label="优先级" width="100" />
          <el-table-column label="来源" min-width="180">
            <template #default="scope">
              {{ scope.row.source?.system_type || '-' }} / {{ scope.row.source?.endpoint_id || scope.row.source?.pattern || '-' }}
            </template>
          </el-table-column>
          <el-table-column label="目标端点" min-width="220">
            <template #default="scope">
              <el-space wrap>
                <el-tag v-for="target in scope.row.targets || []" :key="target.endpoint_id" effect="plain">
                  {{ endpointNames.get(target.endpoint_id) || target.endpoint_id }}
                </el-tag>
              </el-space>
            </template>
          </el-table-column>
          <el-table-column label="状态" width="120">
            <template #default="scope">
              <el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="320" fixed="right">
            <template #default="scope">
              <el-space wrap>
                <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
                <el-button v-if="scope.row.enabled" link type="warning" @click="toggleRoute(scope.row, false)">停用</el-button>
                <el-button v-else link type="success" @click="toggleRoute(scope.row, true)">启用</el-button>
                <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="currentPage" background layout="pager" :page-size="pageSize" :pager-count="7" :total="total" />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.id ? '编辑路由' : '新建路由'" size="720px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item label="名称"><el-input v-model="form.name" /></el-form-item>
            <el-form-item label="优先级"><el-select v-model="form.priority"><el-option v-for="item in routePriorityOptions" :key="item" :label="String(item)" :value="item" /></el-select></el-form-item>
            <el-form-item label="来源系统类型"><el-select v-model="form.system_type" clearable><el-option v-for="item in systemTypeOptions" :key="item" :label="item" :value="item" /></el-select></el-form-item>
            <el-form-item label="来源端点 ID"><el-input v-model="form.source_endpoint_id" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="来源匹配表达式"><el-input v-model="form.source_pattern" placeholder="^his-.*" /></el-form-item>
            <el-form-item label="协议"><el-select v-model="form.protocol" clearable><el-option v-for="item in protocolOptions" :key="item" :label="item" :value="item" /></el-select></el-form-item>
            <el-form-item label="消息类型"><el-input v-model="form.message_type" placeholder="ADT_A01" /></el-form-item>
            <el-form-item label="状态"><el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" /></el-form-item>
          </div>

          <el-form-item label="目标端点 ID 列表（逗号分隔）"><el-input v-model="form.target_ids" placeholder="endpoint-a, endpoint-b" /></el-form-item>
          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="2" /></el-form-item>
          <el-form-item label="Metadata JSON" class="code-block"><el-input v-model="form.metadataText" type="textarea" /></el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>
    </div>
  `,
};

const MessagesPage = {
  name: 'MessagesPage',
  setup() {
    const filters = reactive({ page: 1, page_size: 20, status: '', protocol: '' });
    const resource = useAsyncResource(async () => apiRequest('/messages', { query: filters }), { items: [], total: 0 });
    const detailVisible = ref(false);
    const detail = ref(null);
    const replayLoadingId = ref('');
    const pageCount = computed(() => Math.max(1, Math.ceil((resource.state.value.total || 0) / filters.page_size)));

    async function openDetail(item) {
      try {
        detail.value = await apiRequest(`/messages/${item.id}`);
        detailVisible.value = true;
      } catch (error) {
        ElMessage.error(error.message || '加载消息详情失败');
      }
    }

    async function reprocess(item) {
      try {
        replayLoadingId.value = item.id;
        await apiRequest(`/messages/${item.id}/reprocess`, { method: 'POST' });
        ElMessage.success('消息已提交重处理');
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '消息重处理失败');
      } finally {
        replayLoadingId.value = '';
      }
    }

    function goFirstPage() {
      filters.page = 1;
    }

    function goLastPage() {
      filters.page = pageCount.value;
    }

    watch(() => ({ ...filters }), resource.refresh, { deep: true });
    watch(() => [filters.status, filters.protocol, filters.page_size], () => {
      filters.page = 1;
    });
    onMounted(resource.refresh);

    return {
      filters,
      ...resource,
      pageCount,
      detailVisible,
      detail,
      replayLoadingId,
      openDetail,
      reprocess,
      goFirstPage,
      goLastPage,
      formatTime,
      humanizeState,
      tagTypeByState,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">消息中心</h2>
            <p class="page-section-subtitle">查看持久化消息、处理状态和重放入口。这里直接对接后台的消息存储与回放能力。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
          </div>
        </div>

        <div class="list-filter-bar">
          <div class="filters-grid">
          <el-form-item label="状态">
            <el-input v-model="filters.status" placeholder="pending / processed / failed" />
          </el-form-item>
          <el-form-item label="协议">
            <el-select v-model="filters.protocol" clearable>
              <el-option v-for="item in protocolOptions" :key="item" :label="item" :value="item" />
            </el-select>
          </el-form-item>
          <el-form-item label="每页数量">
            <el-input-number v-model="filters.page_size" :min="10" :max="100" style="width: 100%;" />
          </el-form-item>
          </div>
        </div>

        <el-table :data="state.value.items || []" stripe v-loading="state.loading" row-key="id" class="list-table">
          <el-table-column prop="id" label="消息ID" min-width="220" />
          <el-table-column prop="protocol" label="协议" width="120" />
          <el-table-column prop="message_type" label="消息类型" width="150" />
          <el-table-column label="状态" width="130">
            <template #default="scope">
              <el-tag :type="tagTypeByState(scope.row.status)">{{ humanizeState(scope.row.status) }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="接收时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.received_at || scope.row.created_at) }}</template>
          </el-table-column>
          <el-table-column label="操作" width="220" fixed="right">
            <template #default="scope">
              <el-space>
                <el-button link type="primary" @click="openDetail(scope.row)">详情</el-button>
                <el-button link type="warning" :loading="replayLoadingId === scope.row.id" @click="reprocess(scope.row)">重处理</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="state.value.total > 0">
          <el-button @click="goFirstPage" :disabled="filters.page <= 1">首页</el-button>
          <el-pagination v-model:current-page="filters.page" background layout="pager" :page-size="filters.page_size" :pager-count="7" :total="state.value.total || 0" />
          <el-button @click="goLastPage" :disabled="filters.page >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ filters.page_size }} 行 / 共 {{ state.value.total || 0 }} 条</span>
        </div>
      </div>

      <el-dialog v-model="detailVisible" title="消息详情" width="900px">
        <el-descriptions v-if="detail" :column="2" border>
          <el-descriptions-item label="消息ID">{{ detail.id }}</el-descriptions-item>
          <el-descriptions-item label="协议">{{ detail.protocol }}</el-descriptions-item>
          <el-descriptions-item label="状态">{{ detail.status }}</el-descriptions-item>
          <el-descriptions-item label="消息类型">{{ detail.message_type || '-' }}</el-descriptions-item>
          <el-descriptions-item label="来源系统">{{ detail.source_system || '-' }}</el-descriptions-item>
          <el-descriptions-item label="接收时间">{{ formatTime(detail.received_at || detail.created_at) }}</el-descriptions-item>
        </el-descriptions>
        <el-form-item label="Payload" class="code-block" style="margin-top: 16px;">
          <el-input :model-value="stringifyPretty(detail?.payload || {})" type="textarea" readonly />
        </el-form-item>
        <el-form-item label="Metadata" class="code-block">
          <el-input :model-value="stringifyPretty(detail?.metadata || {})" type="textarea" readonly />
        </el-form-item>
      </el-dialog>
    </div>
  `,
};

const DlqPage = {
  name: 'DlqPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/dlq'), { items: [], total: 0 });
    const actionLoadingId = ref('');
    const filters = reactive({ keyword: '', reason: '' });

    const reasonOptions = computed(() => uniqueValues(resource.state.value.items || [], 'reason'));
    const filteredItems = computed(() => {
      return (resource.state.value.items || []).filter((item) => {
        if (filters.reason && item.reason !== filters.reason) {
          return false;
        }
        return matchesKeyword(item, filters.keyword, ['id', 'message_id', 'reason', 'error_detail']);
      });
    });
    const pager = usePagedList(filteredItems, { minTableHeight: 300 });

    async function reprocess(item) {
      try {
        actionLoadingId.value = `reprocess:${item.id}`;
        await apiRequest(`/dlq/${item.id}/reprocess`, { method: 'POST' });
        ElMessage.success('死信已提交重放');
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '死信重放失败');
      } finally {
        actionLoadingId.value = '';
      }
    }

    async function remove(item) {
      try {
        actionLoadingId.value = `delete:${item.id}`;
        await apiRequest(`/dlq/${item.id}`, { method: 'DELETE' });
        ElMessage.success('死信已删除');
        await resource.refresh();
      } catch (error) {
        ElMessage.error(error.message || '删除死信失败');
      } finally {
        actionLoadingId.value = '';
      }
    }

    onMounted(resource.refresh);

    return {
      ...resource,
      ...pager,
      actionLoadingId,
      filters,
      reasonOptions,
      filteredItems,
      formatTime,
      humanizeState,
      reprocess,
      remove,
      resetFilters() {
        Object.assign(filters, { keyword: '', reason: '' });
      },
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">死信队列</h2>
            <p class="page-section-subtitle">集中处理无法自动恢复的消息。支持再次投递和人工删除，便于快速收敛积压。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refresh">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
          </div>
        </div>
        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入死信 ID、原消息 ID 或失败原因" clearable />
            </el-form-item>
            <el-form-item label="失败原因">
              <el-select v-model="filters.reason" clearable placeholder="全部原因">
                <el-option v-for="item in reasonOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
          </div>
        </div>
        <el-table :data="pagedItems" stripe v-loading="state.loading" row-key="id" class="list-table" :height="tableHeight">
          <el-table-column prop="id" label="死信ID" min-width="220" />
          <el-table-column prop="message_id" label="原消息ID" min-width="220" />
          <el-table-column prop="reason" label="失败原因" min-width="240" />
          <el-table-column label="入队时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.created_at || scope.row.timestamp) }}</template>
          </el-table-column>
          <el-table-column label="操作" width="220" fixed="right">
            <template #default="scope">
              <el-space>
                <el-button link type="warning" :loading="actionLoadingId === 'reprocess:' + scope.row.id" @click="reprocess(scope.row)">重放</el-button>
                <el-button link type="danger" :loading="actionLoadingId === 'delete:' + scope.row.id" @click="remove(scope.row)">删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="total > 0">
          <el-button @click="goFirstPage" :disabled="currentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="currentPage" background layout="pager" :page-size="pageSize" :pager-count="7" :total="total" />
          <el-button @click="goLastPage" :disabled="currentPage >= pageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ pageSize }} 行 / 共 {{ total }} 条</span>
        </div>
      </div>
    </div>
  `,
};

const AuditPage = {
  name: 'AuditPage',
  setup() {
    const auditResource = useAsyncResource(async () => apiRequest('/audit'), { items: [], total: 0 });
    const traceResource = useAsyncResource(async () => apiRequest('/trace'), { items: [], total: 0 });

    onMounted(async () => {
      await Promise.all([auditResource.refresh(), traceResource.refresh()]);
    });

    return {
      auditResource,
      traceResource,
      formatTime,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">审计日志</h2>
            <p class="page-section-subtitle">记录管理操作和关键运行行为，便于追责与回溯。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="auditResource.refresh">刷新审计</el-button>
            <el-button @click="traceResource.refresh">刷新追踪</el-button>
          </div>
        </div>
        <el-table :data="auditResource.state.value.items || []" stripe v-loading="auditResource.state.loading">
          <el-table-column prop="id" label="审计ID" min-width="200" />
          <el-table-column prop="action" label="动作" min-width="160" />
          <el-table-column prop="actor" label="执行者" width="160" />
          <el-table-column label="时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.timestamp || scope.row.created_at) }}</template>
          </el-table-column>
        </el-table>
      </div>
      <div class="page-card">
        <h2 class="page-section-title">调用追踪</h2>
        <el-table :data="traceResource.state.value.items || []" stripe v-loading="traceResource.state.loading">
          <el-table-column prop="trace_id" label="Trace ID" min-width="220" />
          <el-table-column prop="span_id" label="Span ID" min-width="180" />
          <el-table-column prop="name" label="名称" min-width="180" />
          <el-table-column label="时间" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.timestamp || scope.row.created_at) }}</template>
          </el-table-column>
        </el-table>
      </div>
    </div>
  `,
};

const WorkflowsPage = {
  name: 'WorkflowsPage',
  setup() {
    const resource = useAsyncResource(async () => apiRequest('/workflows'), { items: [], total: 0 });
    const instanceResource = useAsyncResource(async () => apiRequest('/workflow-instances'), { items: [], total: 0 });
    const customProtocolResource = useAsyncResource(async () => apiRequest('/custom-protocols'), { items: [], total: 0 });
    const drawerVisible = ref(false);
    const startDrawerVisible = ref(false);
    const detailVisible = ref(false);
    const submitting = ref(false);
    const startSubmitting = ref(false);
    const instanceActionId = ref('');
    const filters = reactive({ keyword: '', enabled: '', step_type: '' });
    const instanceFilters = reactive({ keyword: '', status: '' });
    const form = reactive(defaultWorkflowForm());
    const startForm = reactive(defaultWorkflowStartForm());
    const instanceDetail = ref(null);

    async function refreshAll() {
      await Promise.all([resource.refresh(), instanceResource.refresh()]);
    }

    function resetForm(item = null) {
      Object.assign(form, defaultWorkflowForm(), item ? workflowFormFromItem(item) : {});
    }

    function resetStartForm(workflowId = '') {
      Object.assign(startForm, defaultWorkflowStartForm(workflowId));
    }

    function openCreate() {
      resetForm();
      drawerVisible.value = true;
    }

    function openEdit(item) {
      resetForm(item);
      drawerVisible.value = true;
    }

    function openStart(item) {
      resetStartForm(item.id);
      startDrawerVisible.value = true;
    }

    async function save() {
      try {
        submitting.value = true;
        const isUpdate = !!form.id;
        const payload = workflowPayloadFromForm(form, isUpdate);
        if (isUpdate) {
          await apiRequest(`/workflows/${form.id}`, {
            method: 'PUT',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('工作流已更新');
        } else {
          await apiRequest('/workflows', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: createJsonBody(payload),
          });
          ElMessage.success('工作流已创建');
        }
        drawerVisible.value = false;
        await refreshAll();
      } catch (error) {
        ElMessage.error(error.message || '保存工作流失败');
      } finally {
        submitting.value = false;
      }
    }

    async function remove(item) {
      try {
        await ElMessageBox.confirm(`确认删除工作流 ${item.name || item.id} 吗？`, '删除工作流', { type: 'warning' });
        await apiRequest(`/workflows/${item.id}`, { method: 'DELETE' });
        ElMessage.success('工作流已删除');
        await refreshAll();
      } catch (error) {
        if (error !== 'cancel') {
          ElMessage.error(error.message || '删除工作流失败');
        }
      }
    }

    async function toggleEnabled(item, enabled) {
      try {
        await apiRequest(`/workflows/${item.id}`, {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: createJsonBody({ enabled }),
        });
        ElMessage.success(enabled ? '工作流已启用' : '工作流已停用');
        await refreshAll();
      } catch (error) {
        ElMessage.error(error.message || '更新工作流状态失败');
      }
    }

    async function startInstance() {
      try {
        startSubmitting.value = true;
        const payload = workflowStartPayloadFromForm(startForm);
        await apiRequest(`/workflows/${startForm.workflow_id}/start`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: createJsonBody(payload),
        });
        ElMessage.success('工作流实例已启动');
        startDrawerVisible.value = false;
        await refreshAll();
      } catch (error) {
        ElMessage.error(error.message || '启动工作流实例失败');
      } finally {
        startSubmitting.value = false;
      }
    }

    async function openInstanceDetail(item) {
      try {
        instanceDetail.value = await apiRequest(`/workflow-instances/${item.id}`);
        detailVisible.value = true;
      } catch (error) {
        ElMessage.error(error.message || '加载实例详情失败');
      }
    }

    async function runInstanceAction(item, action) {
      try {
        instanceActionId.value = `${action}:${item.id}`;
        await apiRequest(`/workflow-instances/${item.id}/${action}`, { method: 'POST' });
        ElMessage.success(`实例${humanizeState(action)}已提交`);
        await refreshAll();
      } catch (error) {
        ElMessage.error(error.message || '实例操作失败');
      } finally {
        instanceActionId.value = '';
      }
    }

    onMounted(async () => {
      await Promise.all([refreshAll(), customProtocolResource.refresh()]);
    });

    const stepTypeOptions = computed(() => {
      return uniqueValues(
        (resource.state.value.items || []).flatMap((item) => item.steps || []).map((step) => ({ type: workflowStepType(step) })),
        'type',
      );
    });

    const instanceStatusOptions = computed(() => uniqueValues(instanceResource.state.value.items || [], 'status'));

    const filteredItems = computed(() => {
      return (resource.state.value.items || []).filter((item) => {
        if (filters.enabled !== '') {
          const expected = filters.enabled === 'true';
          if (!!item.enabled !== expected) {
            return false;
          }
        }
        if (filters.step_type && !(item.steps || []).some((step) => workflowStepType(step) === filters.step_type)) {
          return false;
        }
        return matchesKeyword(item, filters.keyword, [
          'id',
          'name',
          'description',
          (value) => (value.steps || []).map((step) => [step.id, step.name, workflowStepType(step)]),
        ]);
      });
    });

    const filteredInstances = computed(() => {
      return (instanceResource.state.value.items || []).filter((item) => {
        if (instanceFilters.status && item.status !== instanceFilters.status) {
          return false;
        }
        return matchesKeyword(item, instanceFilters.keyword, ['id', 'workflow_id', 'current_step_id', 'status']);
      });
    });

    const workflowPager = usePagedList(filteredItems, { reservedHeight: 470, compactReservedHeight: 410 });
    const instancePager = usePagedList(filteredInstances, { reservedHeight: 430, compactReservedHeight: 390, minTableHeight: 300 });

    function resetFilters() {
      Object.assign(filters, { keyword: '', enabled: '', step_type: '' });
    }

    function resetInstanceFilters() {
      Object.assign(instanceFilters, { keyword: '', status: '' });
    }

    return {
      ...resource,
      instanceResource,
      customProtocolResource,
      drawerVisible,
      startDrawerVisible,
      detailVisible,
      submitting,
      startSubmitting,
      instanceActionId,
      filters,
      instanceFilters,
      form,
      startForm,
      instanceDetail,
      filteredItems,
      filteredInstances,
      stepTypeOptions,
      instanceStatusOptions,
      workflowCurrentPage: workflowPager.currentPage,
      workflowPageSize: workflowPager.pageSize,
      workflowTotal: workflowPager.total,
      workflowPageCount: workflowPager.pageCount,
      workflowPagedItems: workflowPager.pagedItems,
      workflowTableHeight: workflowPager.tableHeight,
      workflowGoFirstPage: workflowPager.goFirstPage,
      workflowGoLastPage: workflowPager.goLastPage,
      instanceCurrentPage: instancePager.currentPage,
      instancePageSize: instancePager.pageSize,
      instanceTotal: instancePager.total,
      instancePageCount: instancePager.pageCount,
      instancePagedItems: instancePager.pagedItems,
      instanceTableHeight: instancePager.tableHeight,
      instanceGoFirstPage: instancePager.goFirstPage,
      instanceGoLastPage: instancePager.goLastPage,
      openCreate,
      openEdit,
      openStart,
      save,
      remove,
      toggleEnabled,
      startInstance,
      openInstanceDetail,
      runInstanceAction,
      refreshAll,
      resetFilters,
      resetInstanceFilters,
      workflowStepType,
      formatTime,
      tagTypeByState,
      canPauseInstance,
      canResumeInstance,
      canCancelInstance,
      canCompensateInstance,
      stringifyPretty,
      protocolOptions,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">工作流定义</h2>
            <p class="page-section-subtitle">这里已经接到真实后端 CRUD 和实例启动接口。定义存 PostgreSQL，实例由服务端共享执行器驱动，并同步到控制面查询接口。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="refreshAll">刷新</el-button>
            <el-button @click="resetFilters">清空筛选</el-button>
            <el-button type="primary" @click="openCreate">新建工作流</el-button>
          </div>
        </div>

        <el-alert
          title="步骤与补偿策略采用 JSON 编辑，字段与 hsb-core 工作流模型对齐；适合先把后端定义跑通，再迭代可视化编排器。"
          type="info"
          show-icon
          :closable="false"
          style="margin-bottom: 16px;"
        />

        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="filters.keyword" placeholder="输入工作流名称、ID、描述或步骤" clearable />
            </el-form-item>
            <el-form-item label="步骤类型">
              <el-select v-model="filters.step_type" clearable placeholder="全部类型">
                <el-option v-for="item in stepTypeOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item label="启用状态">
              <el-select v-model="filters.enabled" clearable placeholder="全部状态">
                <el-option label="启用" value="true" />
                <el-option label="停用" value="false" />
              </el-select>
            </el-form-item>
          </div>
        </div>

        <el-table :data="workflowPagedItems" stripe v-loading="state.loading" row-key="id" class="list-table" :height="workflowTableHeight">
          <el-table-column prop="name" label="名称" min-width="180" />
          <el-table-column prop="id" label="ID" min-width="220" />
          <el-table-column prop="version" label="版本" width="100" />
          <el-table-column label="步骤数" width="100">
            <template #default="scope">{{ (scope.row.steps || []).length }}</template>
          </el-table-column>
          <el-table-column label="状态" width="120">
            <template #default="scope">
              <el-tag :type="scope.row.enabled ? 'success' : 'info'">{{ scope.row.enabled ? '启用' : '停用' }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column label="最近更新" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.updated_at) }}</template>
          </el-table-column>
          <el-table-column label="步骤摘要" min-width="260">
            <template #default="scope">
              <el-space wrap>
                <el-tag v-for="step in (scope.row.steps || []).slice(0, 4)" :key="step.id" effect="plain">
                  {{ step.name }} / {{ workflowStepType(step) }}
                </el-tag>
              </el-space>
            </template>
          </el-table-column>
          <el-table-column label="操作" width="320" fixed="right">
            <template #default="scope">
              <el-space wrap>
                <el-button link type="primary" @click="openEdit(scope.row)">编辑</el-button>
                <el-button link type="success" @click="openStart(scope.row)">启动实例</el-button>
                <el-button v-if="scope.row.enabled" link type="warning" @click="toggleEnabled(scope.row, false)">停用</el-button>
                <el-button v-else link type="success" @click="toggleEnabled(scope.row, true)">启用</el-button>
                <el-button link type="danger" @click="remove(scope.row)">删除</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="workflowTotal > 0">
          <el-button @click="workflowGoFirstPage" :disabled="workflowCurrentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="workflowCurrentPage" background layout="pager" :page-size="workflowPageSize" :pager-count="7" :total="workflowTotal" />
          <el-button @click="workflowGoLastPage" :disabled="workflowCurrentPage >= workflowPageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ workflowPageSize }} 行 / 共 {{ workflowTotal }} 条</span>
        </div>
      </div>

      <div class="page-card">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">工作流实例</h2>
            <p class="page-section-subtitle">查看实例运行状态，并执行暂停、恢复、取消和补偿。当前实例控制基于服务端共享执行器，列表也会同步展示最近的执行快照。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="instanceResource.refresh">刷新实例</el-button>
            <el-button @click="resetInstanceFilters">清空筛选</el-button>
          </div>
        </div>

        <div class="list-filter-bar">
          <div class="filters-grid">
            <el-form-item label="搜索">
              <el-input v-model="instanceFilters.keyword" placeholder="输入实例 ID、工作流 ID 或当前步骤" clearable />
            </el-form-item>
            <el-form-item label="状态">
              <el-select v-model="instanceFilters.status" clearable placeholder="全部状态">
                <el-option v-for="item in instanceStatusOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
          </div>
        </div>

        <el-table :data="instancePagedItems" stripe v-loading="instanceResource.state.loading" row-key="id" class="list-table" :height="instanceTableHeight">
          <el-table-column prop="workflow_id" label="工作流 ID" min-width="180" />
          <el-table-column prop="id" label="实例 ID" min-width="220" />
          <el-table-column label="状态" width="140">
            <template #default="scope">
              <el-tag :type="tagTypeByState(scope.row.status)">{{ scope.row.status }}</el-tag>
            </template>
          </el-table-column>
          <el-table-column prop="current_step_id" label="当前步骤" min-width="160" />
          <el-table-column label="历史步骤数" width="120">
            <template #default="scope">{{ (scope.row.step_history || []).length }}</template>
          </el-table-column>
          <el-table-column label="最近更新" min-width="180">
            <template #default="scope">{{ formatTime(scope.row.updated_at) }}</template>
          </el-table-column>
          <el-table-column label="操作" width="360" fixed="right">
            <template #default="scope">
              <el-space wrap>
                <el-button link type="primary" @click="openInstanceDetail(scope.row)">详情</el-button>
                <el-button
                  v-if="canPauseInstance(scope.row.status)"
                  link
                  type="warning"
                  :loading="instanceActionId === 'pause:' + scope.row.id"
                  @click="runInstanceAction(scope.row, 'pause')"
                >暂停</el-button>
                <el-button
                  v-if="canResumeInstance(scope.row.status)"
                  link
                  type="success"
                  :loading="instanceActionId === 'resume:' + scope.row.id"
                  @click="runInstanceAction(scope.row, 'resume')"
                >恢复</el-button>
                <el-button
                  v-if="canCancelInstance(scope.row.status)"
                  link
                  type="danger"
                  :loading="instanceActionId === 'cancel:' + scope.row.id"
                  @click="runInstanceAction(scope.row, 'cancel')"
                >取消</el-button>
                <el-button
                  v-if="canCompensateInstance(scope.row.status)"
                  link
                  type="info"
                  :loading="instanceActionId === 'compensate:' + scope.row.id"
                  @click="runInstanceAction(scope.row, 'compensate')"
                >补偿</el-button>
              </el-space>
            </template>
          </el-table-column>
        </el-table>
        <div class="list-pager" v-if="instanceTotal > 0">
          <el-button @click="instanceGoFirstPage" :disabled="instanceCurrentPage <= 1">首页</el-button>
          <el-pagination v-model:current-page="instanceCurrentPage" background layout="pager" :page-size="instancePageSize" :pager-count="7" :total="instanceTotal" />
          <el-button @click="instanceGoLastPage" :disabled="instanceCurrentPage >= instancePageCount">尾页</el-button>
          <span class="list-pager__summary">每页 {{ instancePageSize }} 行 / 共 {{ instanceTotal }} 条</span>
        </div>
      </div>

      <el-drawer v-model="drawerVisible" :title="form.id ? '编辑工作流' : '新建工作流'" size="860px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item label="工作流 ID"><el-input v-model="form.id" :disabled="!!form.id" placeholder="wf_patient_registration" /></el-form-item>
            <el-form-item label="名称"><el-input v-model="form.name" /></el-form-item>
            <el-form-item label="版本"><el-input-number v-model="form.version" :min="1" :step="1" style="width: 100%;" /></el-form-item>
            <el-form-item label="状态"><el-switch v-model="form.enabled" active-text="启用" inactive-text="停用" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="全局超时(ms)"><el-input-number v-model="form.timeout_ms" :min="1000" :step="1000" style="width: 100%;" /></el-form-item>
            <el-form-item label="实例超时(s)"><el-input-number v-model="form.instance_timeout_secs" :min="1" :step="60" style="width: 100%;" /></el-form-item>
            <el-form-item label="最大并发实例"><el-input-number v-model="form.max_concurrent_instances" :min="1" :step="1" style="width: 100%;" /></el-form-item>
            <el-form-item label="持久化状态"><el-switch v-model="form.persist_state" /></el-form-item>
          </div>

          <div class="filters-grid">
            <el-form-item label="支持暂停恢复"><el-switch v-model="form.pausable" /></el-form-item>
          </div>

          <el-form-item label="描述"><el-input v-model="form.description" type="textarea" :rows="2" /></el-form-item>

          <el-form-item label="补偿策略 JSON" class="code-block">
            <el-input v-model="form.compensationText" type="textarea" placeholder='{"mode":"sequential","timeout_ms":60000,"continue_on_failure":false}' />
          </el-form-item>

          <el-form-item label="步骤定义 JSON" class="code-block">
            <el-input v-model="form.stepsText" type="textarea" />
          </el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="drawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="submitting" @click="save">保存</el-button>
          </div>
        </template>
      </el-drawer>

      <el-drawer v-model="startDrawerVisible" title="启动工作流实例" size="720px">
        <el-form label-position="top">
          <div class="filters-grid">
            <el-form-item label="工作流 ID"><el-input v-model="startForm.workflow_id" disabled /></el-form-item>
            <el-form-item label="来源系统"><el-input v-model="startForm.source_system" /></el-form-item>
            <el-form-item label="目标系统"><el-input v-model="startForm.target_system" /></el-form-item>
            <el-form-item label="协议">
              <el-select v-model="startForm.protocol">
                <el-option v-for="item in protocolOptions" :key="item" :label="item" :value="item" />
              </el-select>
            </el-form-item>
            <el-form-item v-if="startForm.protocol === 'CUSTOM'" label="自定义协议">
              <el-select v-model="startForm.custom_protocol_id" filterable placeholder="请选择已启用的自定义协议">
                <el-option
                  v-for="item in (customProtocolResource.state.value.items || []).filter((protocol) => protocol.enabled)"
                  :key="item.id"
                  :label="item.name || item.id"
                  :value="item.id"
                />
              </el-select>
            </el-form-item>
          </div>
          <div class="filters-grid">
            <el-form-item label="消息类型"><el-input v-model="startForm.message_type" placeholder="ADT_A01" /></el-form-item>
            <el-form-item label="关联 ID"><el-input v-model="startForm.correlation_id" /></el-form-item>
          </div>
          <el-form-item label="Payload JSON" class="code-block">
            <el-input v-model="startForm.payloadText" type="textarea" />
          </el-form-item>
          <el-form-item label="原始报文文本" class="code-block">
            <el-input v-model="startForm.raw_payload_text" type="textarea" placeholder="可选；不填则自动使用 Payload JSON 生成原始内容" />
          </el-form-item>
        </el-form>
        <template #footer>
          <div class="header-actions">
            <el-button @click="startDrawerVisible = false">取消</el-button>
            <el-button type="primary" :loading="startSubmitting" @click="startInstance">启动</el-button>
          </div>
        </template>
      </el-drawer>

      <el-dialog v-model="detailVisible" title="实例详情" width="960px">
        <el-descriptions v-if="instanceDetail" :column="2" border>
          <el-descriptions-item label="实例 ID">{{ instanceDetail.id }}</el-descriptions-item>
          <el-descriptions-item label="工作流 ID">{{ instanceDetail.workflow_id }}</el-descriptions-item>
          <el-descriptions-item label="状态">{{ instanceDetail.status }}</el-descriptions-item>
          <el-descriptions-item label="当前步骤">{{ instanceDetail.current_step_id || '-' }}</el-descriptions-item>
          <el-descriptions-item label="创建时间">{{ formatTime(instanceDetail.created_at) }}</el-descriptions-item>
          <el-descriptions-item label="更新时间">{{ formatTime(instanceDetail.updated_at) }}</el-descriptions-item>
        </el-descriptions>
        <el-form-item label="上下文 JSON" class="code-block" style="margin-top: 16px;">
          <el-input :model-value="stringifyPretty(instanceDetail?.context || {})" type="textarea" readonly />
        </el-form-item>
        <el-form-item label="步骤历史 JSON" class="code-block">
          <el-input :model-value="stringifyPretty(instanceDetail?.step_history || [])" type="textarea" readonly />
        </el-form-item>
      </el-dialog>
    </div>
  `,
};

const SystemPage = {
  name: 'SystemPage',
  setup() {
    const configText = ref('');
    const loading = ref(false);
    const saving = ref(false);

    async function loadConfig() {
      try {
        loading.value = true;
        const result = await apiRequest('/config');
        configText.value = typeof result === 'string' ? result : stringifyPretty(result);
      } catch (error) {
        ElMessage.error(error.message || '加载系统配置失败');
      } finally {
        loading.value = false;
      }
    }

    async function saveConfig() {
      try {
        saving.value = true;
        let payload;
        try {
          payload = JSON.parse(configText.value);
        } catch (error) {
          ElMessage.error('配置必须是合法 JSON');
          saving.value = false;
          return;
        }

        await apiRequest('/config', {
          method: 'PUT',
          headers: { 'Content-Type': 'application/json' },
          body: createJsonBody(payload),
        });
        ElMessage.success('系统配置已更新');
      } catch (error) {
        ElMessage.error(error.message || '保存配置失败');
      } finally {
        saving.value = false;
      }
    }

    async function reloadConfig() {
      try {
        await apiRequest('/config/reload', { method: 'POST' });
        ElMessage.success('配置已触发重载');
      } catch (error) {
        ElMessage.error(error.message || '重载配置失败');
      }
    }

    onMounted(loadConfig);

    return {
      configText,
      loading,
      saving,
      loadConfig,
      saveConfig,
      reloadConfig,
    };
  },
  template: `
    <div class="content-grid">
      <div class="page-card" v-loading="loading">
        <div class="stack-actions">
          <div>
            <h2 class="page-section-title">系统配置</h2>
            <p class="page-section-subtitle">查看和编辑控制面配置，并触发热重载。</p>
          </div>
          <div class="toolbar-right">
            <el-button @click="loadConfig">刷新</el-button>
            <el-button @click="reloadConfig">重载</el-button>
            <el-button type="primary" :loading="saving" @click="saveConfig">保存</el-button>
          </div>
        </div>
        <el-input v-model="configText" type="textarea" class="code-block" />
      </div>
    </div>
  `,
};

const routeDefinitions = [
  { path: '/', redirect: '/dashboard' },
  { path: '/dashboard', component: DashboardPage, meta: { title: '运行态总览', desc: '统一查看路由、端点、消息与系统状态。' } },
  { path: '/organizations', component: OrganizationsPage, meta: { title: '机构管理', desc: '维护机构层级、类型与治理属性。' } },
  { path: '/systems', component: SystemsPage, meta: { title: '系统管理', desc: '维护机构下的业务系统与 topic 约定。' } },
  { path: '/topics', component: TopicsPage, meta: { title: 'Topic 目录', desc: '维护消息主题命名、归属和运行属性。' } },
  { path: '/protocols', component: CustomProtocolsPage, meta: { title: '自定义协议', desc: '维护 CUSTOM 协议字段结构并供端点引用。' } },
  { path: '/endpoints', component: EndpointsPage, meta: { title: '端点管理', desc: '维护系统下的端点、角色、连接与安全策略。' } },
  { path: '/routes', component: RoutesPage, meta: { title: '路由管理', desc: '维护逻辑分发规则与目标端点映射。' } },
  { path: '/messages', component: MessagesPage, meta: { title: '消息中心', desc: '查看持久化消息、状态和重放入口。' } },
  { path: '/dlq', component: DlqPage, meta: { title: '死信队列', desc: '处理人工介入的失败消息。' } },
  { path: '/audit', component: AuditPage, meta: { title: '审计追踪', desc: '查看审计日志与调用链追踪。' } },
  { path: '/workflows', component: WorkflowsPage, meta: { title: '工作流定义', desc: '预览跨系统业务编排模板。' } },
  { path: '/system', component: SystemPage, meta: { title: '系统配置', desc: '维护平台配置并执行热重载。' } },
];

const router = createRouter({
  history: createWebHashHistory(uiBasePath),
  routes: routeDefinitions,
});

const App = {
  name: 'App',
  setup() {
    const route = useRoute();
    const routerInstance = useRouter();
    const activeMenu = computed(() => route.path);
    const currentMeta = computed(() => route.meta || {});
    const currentTime = ref(new Date());
    const currentUser = ref({ authenticated: false, user_name: '', sso_enabled: false });

    setInterval(() => {
      currentTime.value = new Date();
    }, 1000);

    function navigate(path) {
      routerInstance.push(path);
    }

    function logout() {
      window.location.href = withRoutePrefix('/auth/logout');
    }

    async function loadCurrentUser() {
      try {
        const response = await fetch(withRoutePrefix('/auth/me'), { headers: { Accept: 'application/json' } });
        if (response.ok) {
          currentUser.value = await response.json();
        }
      } catch (_) {
        currentUser.value = { authenticated: false, user_name: '', sso_enabled: false };
      }
    }

    onMounted(loadCurrentUser);

    return {
      navGroups,
      activeMenu,
      currentMeta,
      currentTime,
      currentUser,
      navigate,
      logout,
    };
  },
  template: `
    <div class="shell">
      <el-container>
        <el-aside width="290px" class="shell__aside">
          <div class="brand">
            <div class="brand__eyebrow">Nexus HSB</div>
            <div class="brand__title">集成总线管理台</div>
            <div class="brand__subtitle">统一入口由 axum 暴露，控制面与运行面在同一出口下协同工作。</div>
          </div>
          <el-menu :default-active="activeMenu" @select="navigate">
            <el-sub-menu v-for="group in navGroups" :key="group.index" :index="group.index">
              <template #title>{{ group.title }}</template>
              <el-menu-item v-for="item in group.children" :key="item.index" :index="item.index">
                {{ item.title }}
              </el-menu-item>
            </el-sub-menu>
          </el-menu>
          <div class="menu-note">
            当前入口:
            <div>/ui/ 管理台</div>
            <div>/api/v1 控制面 API</div>
            <div>/api/messages/inbound 业务接入</div>
          </div>
        </el-aside>

        <el-container style="padding-left: 18px;">
          <el-header class="shell__header" height="auto">
            <div class="stack-actions" style="margin-bottom: 0;">
              <div>
                <h1 class="shell__title">{{ currentMeta.title || 'Nexus HSB Console' }}</h1>
                <p class="shell__desc">{{ currentMeta.desc || '面向医疗集成总线的配置、运行与运维工作台。' }}</p>
              </div>
              <div class="header-actions">
                <el-tag type="success" size="large">axum 统一出口</el-tag>
                <el-tag effect="plain">{{ currentUser.user_name || '未登录' }}</el-tag>
                <el-button v-if="currentUser.sso_enabled" link type="primary" @click="logout">退出</el-button>
                <el-tag effect="plain">{{ currentTime.toLocaleString('zh-CN') }}</el-tag>
              </div>
            </div>
          </el-header>
          <el-main style="padding: 0; overflow: visible;">
            <router-view />
          </el-main>
        </el-container>
      </el-container>
    </div>
  `,
};

createApp(App).use(router).use(ElementPlus).mount('#app');