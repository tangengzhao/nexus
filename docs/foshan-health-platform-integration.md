# 佛山市卫健统一接口接入配置说明

本文记录依据《佛山市卫生健康统一接口方案 v2.5 试行》在 Nexus HSB 中初始化的机构、系统、生产者端点、自定义协议和 API Topic 目录。该配置用于接收佛山市卫生健康移动综合服务平台发起的业务请求，并为后续路由到院内系统提供统一目录。

## 已创建目录记录

| 类型 | ID | 名称 | 说明 |
| --- | --- | --- | --- |
| 机构 | `foshan-wjw` | 佛山市卫生健康局 | 机构类型为 `GOVERNMENT_DEPARTMENT`，记录文档版本 `v2.5`。 |
| 系统 | `foshan-health-mobile-platform` | 佛山市卫生健康移动综合服务平台 | 系统类型为 `PROVINCIAL_PLATFORM`，Topic 命名空间为 `foshan`。 |
| 生产者端点 | `foshan-health-platform-producer` | 佛山市卫健移动平台生产者入口 | Endpoint 角色为 `PRODUCER`，协议为 `CUSTOM`，关联自定义协议 `foshan-health-rest-json-v25`。 |
| 自定义协议 | `foshan-health-rest-json-v25` | 佛山市卫健统一接口 REST JSON v2.5 | 传输提示 `http`，内容类型 `application/json`。 |

## 协议要求

接口统一使用 HTTPS + POST + JSON，字符集为 UTF-8。业务请求体以各接口章节定义为准，HSB 目录层统一记录公共 Header、响应 envelope 和常见业务字段。

### 公共 Header

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `God-Portal-Partner-Id` | 是 | 平台分配的 PartnerId，长度不超过 16 位。 |
| `God-Portal-Timestamp` | 是 | 毫秒级时间戳，有效期 15 分钟。 |
| `God-Portal-Signature` | 是 | 移动综合服务平台章节使用 `SM3(PartnerId + PartnerSecret + Timestamp)`。 |
| `god-portal-partner-id` | 条件必填 | 病案复印平台章节使用的小写 Header。 |
| `god-portal-timestamp` | 条件必填 | 病案复印平台章节使用的小写毫秒时间戳。 |
| `god-portal-signature` | 条件必填 | 病案复印平台章节使用 `HMAC-SHA256(partnerSecret, partnerId + timestamp)`。 |
| `god-portal-request-id` | 否 | 病案复印平台示例中的请求追踪 ID。 |

### 通用响应字段

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `resultCode` | 是 | `0` 表示成功，其他值表示失败或异常。 |
| `resultMessage` | 否 | 返回结果描述，失败时应提供错误原因。 |
| `data` | 否 | 接口业务数据，类型和结构由各 API 章节定义。 |

### 常用业务字段

| 字段 | 说明 |
| --- | --- |
| `cardNo` | 动态电子健康码。 |
| `platformOrderNo` | 平台订单流水号。 |
| `hisOrderNo` | 医院侧订单号。 |
| `departmentCode` | 科室代码。 |
| `doctorCode` | 医生代码。 |

## API Topic 映射

Topic 采用 `domain.service.action.version` 四段结构，版本固定为 `v25`，并在 Topic 属性中保存原始路径、提供方、是否必须对接、调用方向和协议 ID。

| Topic | 原始接口地址 | 提供方 | 接入要求 | 调用方向 |
| --- | --- | --- | --- | --- |
| `foshan.register.get_department_info.v25` | `/API/Register/GetDepartmentInfo` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.get_doctor_info.v25` | `/API/Register/GetDoctorInfo` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.get_today_schedule_source.v25` | `/API/Register/GetTodayDoctorScheduleSourceInfo` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.add_register_order.v25` | `/API/Register/AddRegisterOrder` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.cancel_lock_register_order.v25` | `/API/Register/CancelLockRegisterOrder` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.confirm_register_pay_order.v25` | `/API/Register/ConfirmRegisterPayOrder` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.get_register_pay_order_status.v25` | `/API/Register/GetRegisterPayOrderStatus` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.get_booking_schedule_source.v25` | `/API/Register/GetBookingDoctorScheduleSourceInfo` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.cancel_register_order.v25` | `/API/Register/CancelRegisterOrder` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.refund_notice.v25` | `/API/Register/RefundNotice` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.get_register_wait_info.v25` | `/API/Register/GetRegisterWaitInfo` | 医院 | 必须提供；就诊报到场景有此业务的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.get_setmeths.v25` | `/API/Register/GetSetmeths` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.get_setmeths.v25` | `/API/Outpatient/GetSetmeths` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.get_pay_record_list.v25` | `/API/Outpatient/GetOutpatientPayRecordList` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.get_pay_record_detail.v25` | `/API/Outpatient/GetOutpatientPayRecordDetail` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.comp_bill_fee.v25` | `/API/Outpatient/CompBillFee` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.cancel_comp_bill_fee.v25` | `/API/Outpatient/CancelCompBillFee` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.add_pay_order.v25` | `/API/Outpatient/AddOutpatientPayOrder` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.outpatient.get_pay_order_status.v25` | `/API/Outpatient/GetOutpatientPayOrderStatus` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.report.get_exam_list.v25` | `/API/Report/GetExamList` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.report.get_exam_detail.v25` | `/API/Report/GetExamDetail` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.report.get_inspect_list.v25` | `/API/Report/GetInspectList` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.report.get_inspect_detail.v25` | `/API/Report/GetInspectDetail` | 医院 | 必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.register_check.v25` | `/API/Register/RegisterCheck` | 医院 | 有此业务的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.register.stop_register_order.v25` | `/API/Register/StopRegisterOrder` | 市平台 | 必须对接 | `platform_to_hsb` |
| `foshan.register.push_register_queue.v25` | `/API/Register/PushRegisterQueue` | 市平台 | 必须对接 | `platform_to_hsb` |
| `foshan.outpatient.push_payment.v25` | `/API/Outpatient/PushOutpatientPayment` | 市平台 | 必须对接 | `platform_to_hsb` |
| `foshan.report.push_report.v25` | `/API/Report/PushReport` | 市平台 | 必须对接 | `platform_to_hsb` |
| `foshan.outpatient.push_recipe.v25` | `/API/Outpatient/PushRecipeOutpatient` | 市平台 | 必须对接 | `platform_to_hsb` |
| `foshan.register.refund.v25` | `/API/Register/Refund` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.outpatient.refund.v25` | `/API/Outpatient/Refund` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.hospitalization.refund.v25` | `/API/Hospitalization/Refund` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.bill.get_date_bill.v25` | `/API/Bill/GetDateBill` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.register.get_order_status.v25` | `/API/Register/GetOrderStatus` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.outpatient.get_order_status.v25` | `/API/Outpatient/GetOrderStatus` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.hospitalization.get_order_status.v25` | `/API/Hospitalization/GetOrderStatus` | 市平台 | 必须对接 | `hsb_to_platform` |
| `foshan.report.get_hospitalization_info.v25` | `/API/Report/GetHospitalizationInfo` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.hospitalization.get_fee_cost_list.v25` | `/API/Hospitalization/GetHospitalizationFeeCostList` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.hospitalization.add_pay_order.v25` | `/API/Hospitalization/AddHospitalizationPayOrder` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.hospitalization.get_pay_order_status.v25` | `/API/Hospitalization/GetHospitalizationPayOrderStatus` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.medicalrecord.get_user_info.v25` | `/api/getuserinfo` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.medicalrecord.get_outpatient_visit_list.v25` | `/api/getoutpatientvisitlist` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.medicalrecord.get_hospitalization_list.v25` | `/api/gethospitalizationlist` | 医院 | 有对接意向的必须提供 | `platform_to_hsb_to_hospital` |
| `foshan.order.get_order_info.v25` | `/api/OrderMessage/GetOrderInfo` | 市平台 | 有对接意向的必须对接 | `hsb_to_platform` |

## 后续路由建议

当前初始化只建立机构、系统、生产者端点、自定义协议和 API 目录。实际联调时，应按医院系统能力继续创建消费者 Endpoint，并为 `platform_to_hsb_to_hospital` 方向的 Topic 创建 Route，把佛山市平台请求转发到对应医院 HIS/LIS/RIS/EMR 或其他业务系统。

对于 `hsb_to_platform` 方向的接口，需要再补充佛山市平台的消费者端点或专用出站连接配置，并在凭据系统中维护 PartnerSecret。生产环境不得把 PartnerSecret 明文写入 Endpoint 属性或文档。
